use std::{
    str::FromStr,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{SharedAppState, app_auth::verify_canonical_request, limits};
use axum::{
    extract::ConnectInfo,
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use blake3::Hasher;
use hex::ToHex;
use iroha_core::state::WorldReadOnly;
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    content::{
        ContentAuthMode, ContentBundleManifest, ContentBundleRecord, ContentDaReceipt, ContentRange,
    },
    da::types::BlobDigest,
};
use iroha_logger::{error, info};

#[derive(Debug)]
pub enum ContentError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound,
    RateLimited,
    Internal(String),
    RangeNotSatisfiable { total_len: u64 },
}

const CONTENT_RECEIPT_HEADER: &str = "sora-content-receipt";

impl IntoResponse for ContentError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg).into_response(),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, msg).into_response(),
            Self::NotFound => StatusCode::NOT_FOUND.into_response(),
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS.into_response(),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response(),
            Self::RangeNotSatisfiable { total_len } => {
                let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
                response.headers_mut().insert(
                    header::CONTENT_RANGE,
                    HeaderValue::from_str(&format!("bytes */{total_len}"))
                        .unwrap_or_else(|_| HeaderValue::from_static("bytes */0")),
                );
                response
            }
        }
    }
}

/// GET /v2/content/{bundle}/{path...}
#[allow(clippy::too_many_lines)]
pub async fn handle_get_content(
    axum::extract::State(app): axum::extract::State<SharedAppState>,
    axum::extract::Path((bundle_hex, path)): axum::extract::Path<(String, String)>,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    method: Method,
    uri: Uri,
) -> Result<Response, ContentError> {
    let start = Instant::now();
    let mut bytes_served = 0u64;
    let mut outcome_hint: Option<&'static str> = None;

    let result: Result<Response, ContentError> = async {
        let bundle_id = match parse_bundle_id(&bundle_hex) {
            Ok(id) => id,
            Err(err) => {
                outcome_hint = Some("bad_request");
                return Err(err);
            }
        };
        let key = content_rate_key(
            &headers,
            Some(remote.ip()),
            bundle_hex.as_str(),
            app.require_api_token,
        );
        if !limits::allow_conditionally(&app.rate_limiter, &key, true).await {
            outcome_hint = Some("rate_limited");
            return Err(ContentError::RateLimited);
        }
        if !limits::allow_conditionally(&app.content_request_limiter, &key, true).await {
            outcome_hint = Some("rate_limited");
            return Err(ContentError::RateLimited);
        }

        let (bundle, entry, range_spec) = {
            let world = app.state.world_view();
            let current_height = app.state.committed_height() as u64;

            let Some(bundle) =
                mv::storage::StorageReadOnly::get(world.content_bundles(), &bundle_id).cloned()
            else {
                outcome_hint = Some("not_found");
                return Err(ContentError::NotFound);
            };

            if is_bundle_expired(current_height, bundle.expires_at_height) {
                outcome_hint = Some("not_found");
                return Err(ContentError::NotFound);
            }

            let Some(entry) = bundle.files.iter().find(|f| f.path == path).cloned() else {
                outcome_hint = Some("not_found");
                return Err(ContentError::NotFound);
            };

            if let Err(err) = enforce_auth(&bundle.manifest, &app.state, &headers, &method, &uri) {
                outcome_hint = Some("auth_failed");
                return Err(err);
            }

            if let Err(err) = enforce_pow(&app.content_config.pow, &headers, &bundle_id, &path) {
                outcome_hint = Some(match &err {
                    ContentError::Unauthorized(_) => "pow_required",
                    ContentError::Forbidden(_) | ContentError::BadRequest(_) => "pow_invalid",
                    _ => "pow_error",
                });
                return Err(err);
            }

            let chunk_size = u64::from(bundle.chunk_size);
            if chunk_size == 0 {
                outcome_hint = Some("internal");
                return Err(ContentError::Internal(
                    "content chunk size must be greater than zero".to_string(),
                ));
            }

            let range_spec = apply_range(entry.length, headers.get(header::RANGE))?;

            (bundle, entry, range_spec)
        };
        if !limits::allow_cost_conditionally(
            &app.content_egress_limiter,
            &key,
            range_spec.content_length,
            true,
        )
        .await
        {
            outcome_hint = Some("rate_limited");
            return Err(ContentError::RateLimited);
        }

        let body = if range_spec.content_length == 0 {
            Vec::new()
        } else {
            let world = app.state.world_view();
            match assemble_file_range(&world, &bundle, &entry, &range_spec) {
                Ok(body) => body,
                Err(err) => {
                    error!(
                        ?err,
                        bundle = %bundle_id,
                        path,
                        "failed to assemble content file range"
                    );
                    outcome_hint = Some(err.outcome());
                    return Err(ContentError::Internal(err.message().to_string()));
                }
            }
        };

        let receipt_header = encode_receipt_header(&bundle, &entry, &range_spec).ok();
        let status = range_spec.status;
        let content_length = range_spec.content_length;
        let content_range_header = range_spec.content_range.clone();

        bytes_served = content_length;
        let etag = entry.file_hash.encode_hex::<String>();

        let mut response = Response::builder()
            .status(status)
            .body(axum::body::Body::from(body))
            .map_err(|_| ContentError::Internal("failed to build response".to_string()))?;

        let headers_mut = response.headers_mut();
        headers_mut.insert(
            header::ETAG,
            HeaderValue::from_str(&format!("\"{etag}\""))
                .unwrap_or_else(|_| HeaderValue::from_static("\"invalid\"")),
        );
        headers_mut.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        headers_mut.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_str(&bundle.manifest.cache.cache_control_value())
                .unwrap_or_else(|_| HeaderValue::from_static("public, max-age=300")),
        );
        headers_mut.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&mime_for_path(&bundle.manifest, &entry))
                .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
        );
        headers_mut.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&content_length.to_string())
                .unwrap_or_else(|_| HeaderValue::from_static("0")),
        );
        if let Some(ref content_range) = content_range_header {
            headers_mut.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(content_range)
                    .unwrap_or_else(|_| HeaderValue::from_static("bytes */0")),
            );
        }

        if let Some(receipt_header) = receipt_header {
            headers_mut.insert(CONTENT_RECEIPT_HEADER, receipt_header);
        }

        info!(
            bundle = %bundle_id,
            path,
            status = ?range_spec.status,
            cache = ?bundle.manifest.cache,
            auth = ?bundle.manifest.auth,
            "served content bundle file"
        );

        Ok(response)
    }
    .await;

    let outcome = outcome_hint.unwrap_or(match &result {
        Ok(_) => "ok",
        Err(ContentError::Unauthorized(_)) => "auth_required",
        Err(ContentError::Forbidden(_)) => "auth_forbidden",
        Err(ContentError::NotFound) => "not_found",
        Err(ContentError::RateLimited) => "rate_limited",
        Err(ContentError::RangeNotSatisfiable { .. }) => "range_invalid",
        Err(ContentError::BadRequest(_)) => "bad_request",
        Err(ContentError::Internal(_)) => "internal",
    });

    app.telemetry.with_metrics(|telemetry| {
        telemetry.observe_torii_content_request(outcome, bytes_served, start.elapsed());
    });

    result
}

fn parse_bundle_id(bundle_hex: &str) -> Result<Hash, ContentError> {
    let bundle_bytes = hex::decode(bundle_hex.trim_start_matches("0x"))
        .map_err(|_| ContentError::BadRequest("invalid bundle id".to_string()))?;
    if bundle_bytes.len() != Hash::LENGTH {
        return Err(ContentError::BadRequest(
            "invalid bundle id length".to_string(),
        ));
    }
    let mut bundle_arr = [0u8; Hash::LENGTH];
    bundle_arr.copy_from_slice(&bundle_bytes);
    Ok(Hash::prehashed(bundle_arr))
}

fn is_bundle_expired(current_height: u64, expires_at_height: Option<u64>) -> bool {
    matches!(expires_at_height, Some(expiry) if current_height >= expiry)
}

fn content_rate_key(
    headers: &HeaderMap,
    remote: Option<std::net::IpAddr>,
    hint: &str,
    require_api_token: bool,
) -> String {
    limits::key_from_headers(headers, remote, Some(hint), require_api_token)
}

fn enforce_pow(
    pow: &iroha_config::parameters::actual::ContentPow,
    headers: &HeaderMap,
    bundle: &Hash,
    path: &str,
) -> Result<(), ContentError> {
    if pow.difficulty_bits == 0 {
        return Ok(());
    }

    let header_name = header::HeaderName::from_str(&pow.header_name)
        .map_err(|_| ContentError::Internal("invalid content pow header name".to_string()))?;
    let token_value = headers
        .get(&header_name)
        .ok_or_else(|| ContentError::Unauthorized("pow token required".to_string()))?;
    let token_str = token_value
        .to_str()
        .map_err(|_| ContentError::BadRequest("invalid pow token".to_string()))?;
    let token_bytes = hex::decode(token_str)
        .map_err(|_| ContentError::BadRequest("invalid pow token".to_string()))?;

    let mut hasher = Hasher::new();
    hasher.update(bundle.as_ref());
    hasher.update(path.as_bytes());
    hasher.update(&token_bytes);
    let digest = hasher.finalize();

    if leading_zero_bits(digest.as_bytes()) < u32::from(pow.difficulty_bits) {
        return Err(ContentError::Forbidden("pow token invalid".to_string()));
    }

    Ok(())
}

fn enforce_auth(
    manifest: &ContentBundleManifest,
    state: &std::sync::Arc<iroha_core::state::State>,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
) -> Result<(), ContentError> {
    match &manifest.auth {
        ContentAuthMode::Public => Ok(()),
        ContentAuthMode::RoleGate(role) => {
            let account = signed_account(state, headers, method, uri)?;
            let world = state.world_view();
            let has_role = world.account_roles_iter(&account).any(|r| r == role);
            if has_role {
                Ok(())
            } else {
                Err(ContentError::Forbidden("role required".to_string()))
            }
        }
        ContentAuthMode::Sponsor(expected) => {
            let account = signed_account(state, headers, method, uri)?;
            let world = state.world_view();
            let account_entry = world
                .account(&account)
                .map_err(|_| ContentError::Forbidden("account not found".to_string()))?;
            let uaid = account_entry
                .value()
                .uaid()
                .copied()
                .ok_or_else(|| ContentError::Forbidden("uaid required".to_string()))?;
            if uaid == *expected {
                Ok(())
            } else {
                Err(ContentError::Forbidden("uaid mismatch".to_string()))
            }
        }
    }
}

fn signed_account(
    state: &std::sync::Arc<iroha_core::state::State>,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
) -> Result<AccountId, ContentError> {
    match verify_canonical_request(state, headers, method, uri, &[], None) {
        Ok(Some(account)) => Ok(account),
        Ok(None) => Err(ContentError::Unauthorized(
            "signed account headers are required".to_string(),
        )),
        Err(err) => {
            iroha_logger::warn!(?err, "content auth signature rejected");
            Err(ContentError::Unauthorized(
                "invalid request signature".to_string(),
            ))
        }
    }
}

fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut total = 0u32;
    for byte in bytes {
        if *byte == 0 {
            total += 8;
            continue;
        }
        total += byte.leading_zeros();
        break;
    }
    total
}

fn assemble_file_range(
    world: &impl WorldReadOnly,
    bundle: &ContentBundleRecord,
    entry: &iroha_data_model::content::ContentFileEntry,
    range: &RangeSpec,
) -> Result<Vec<u8>, AssembleError> {
    let chunk_size = u64::from(bundle.chunk_size);
    let start = entry
        .offset
        .checked_add(range.start)
        .ok_or(AssembleError::Overflow)?;
    let end_inclusive = entry
        .offset
        .checked_add(range.end)
        .ok_or(AssembleError::Overflow)?;
    let end = end_inclusive
        .checked_add(1)
        .ok_or(AssembleError::Overflow)?;
    assemble_range_from_chunks(chunk_size, &bundle.chunk_hashes, start, end, |hash| {
        mv::storage::StorageReadOnly::get(world.content_chunks(), hash)
            .map(|chunk| chunk.data.as_slice())
    })
}

fn assemble_range_from_chunks<'a, F>(
    chunk_size: u64,
    chunk_hashes: &[[u8; 32]],
    start: u64,
    end: u64,
    mut chunk_lookup: F,
) -> Result<Vec<u8>, AssembleError>
where
    F: FnMut(&[u8; 32]) -> Option<&'a [u8]>,
{
    if start >= end {
        return Err(AssembleError::SliceBounds);
    }
    let start_chunk = start / chunk_size;
    let end_chunk = end.saturating_sub(1) / chunk_size;
    let expected_len = end.checked_sub(start).ok_or(AssembleError::Overflow)? as usize;

    let mut out = Vec::with_capacity(expected_len);
    for idx in start_chunk..=end_chunk {
        let hash = *chunk_hashes
            .get(idx as usize)
            .ok_or(AssembleError::MissingChunk)?;
        let chunk = chunk_lookup(&hash).ok_or(AssembleError::MissingChunk)?;
        let chunk_start = idx.checked_mul(chunk_size).ok_or(AssembleError::Overflow)?;
        let slice_start = start.saturating_sub(chunk_start) as usize;
        let slice_end = (end.min(chunk_start + chunk.len() as u64) - chunk_start) as usize;
        if slice_start > slice_end || slice_end > chunk.len() {
            return Err(AssembleError::SliceBounds);
        }
        out.extend_from_slice(&chunk[slice_start..slice_end]);
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct RangeSpec {
    status: StatusCode,
    content_length: u64,
    content_range: Option<String>,
    start: u64,
    end: u64,
}

#[derive(Debug)]
enum AssembleError {
    MissingChunk,
    SliceBounds,
    Overflow,
}

impl AssembleError {
    fn outcome(&self) -> &'static str {
        match self {
            Self::MissingChunk => "da_missing_chunk",
            Self::SliceBounds | Self::Overflow => "internal",
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::MissingChunk => "missing content chunk",
            Self::SliceBounds => "invalid chunk slice bounds",
            Self::Overflow => "content chunk offset overflow",
        }
    }
}

fn apply_range(
    total_len: u64,
    range_header: Option<&HeaderValue>,
) -> Result<RangeSpec, ContentError> {
    if total_len == 0 {
        if range_header.is_some() {
            return Err(ContentError::RangeNotSatisfiable { total_len: 0 });
        }
        return Ok(RangeSpec {
            status: StatusCode::OK,
            content_length: 0,
            content_range: None,
            start: 0,
            end: 0,
        });
    }
    let Some(raw) = range_header else {
        return Ok(RangeSpec {
            status: StatusCode::OK,
            content_length: total_len,
            content_range: None,
            start: 0,
            end: total_len.saturating_sub(1),
        });
    };

    let range_str = raw
        .to_str()
        .map_err(|_| ContentError::BadRequest("invalid range header".to_string()))?;
    if !range_str.starts_with("bytes=") {
        return Err(ContentError::BadRequest(
            "only bytes ranges are supported".to_string(),
        ));
    }
    let Some((start_str, end_str)) = range_str["bytes=".len()..].split_once('-') else {
        return Err(ContentError::BadRequest("malformed range spec".to_string()));
    };

    let (start, mut end) = if start_str.is_empty() {
        let suffix: u64 = end_str
            .parse()
            .map_err(|_| ContentError::BadRequest("invalid range suffix".to_string()))?;
        let start = total_len.saturating_sub(suffix);
        let end = total_len
            .checked_sub(1)
            .ok_or(ContentError::RangeNotSatisfiable { total_len })?;
        (start, end)
    } else {
        let start: u64 = start_str
            .parse()
            .map_err(|_| ContentError::BadRequest("invalid range start".to_string()))?;
        let end: u64 = if end_str.is_empty() {
            total_len
                .checked_sub(1)
                .ok_or(ContentError::RangeNotSatisfiable { total_len })?
        } else {
            end_str
                .parse()
                .map_err(|_| ContentError::BadRequest("invalid range end".to_string()))?
        };
        (start, end)
    };

    if start >= total_len {
        return Err(ContentError::RangeNotSatisfiable { total_len });
    }

    if end >= total_len {
        end = total_len
            .checked_sub(1)
            .ok_or(ContentError::RangeNotSatisfiable { total_len })?;
    }

    if start > end {
        return Err(ContentError::RangeNotSatisfiable { total_len });
    }

    let content_length = end
        .checked_sub(start)
        .and_then(|len| len.checked_add(1))
        .ok_or(ContentError::RangeNotSatisfiable { total_len })?;
    Ok(RangeSpec {
        status: StatusCode::PARTIAL_CONTENT,
        content_length,
        content_range: Some(format!("bytes {start}-{end}/{total_len}")),
        start,
        end,
    })
}

fn encode_receipt_header(
    bundle: &ContentBundleRecord,
    entry: &iroha_data_model::content::ContentFileEntry,
    range_state: &RangeSpec,
) -> Result<HeaderValue, ContentError> {
    let served_range = if range_state.content_length == 0 {
        None
    } else {
        Some(ContentRange {
            start: range_state.start,
            end: range_state.end,
        })
    };
    let receipt = ContentDaReceipt {
        bundle_id: bundle.bundle_id,
        path: entry.path.clone(),
        file_hash: entry.file_hash,
        served_bytes: range_state.content_length,
        range: served_range,
        chunk_root: BlobDigest::new(bundle.chunk_root),
        stripe_layout: bundle.stripe_layout,
        pdp_commitment: bundle.pdp_commitment.clone(),
        served_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let encoded = norito::to_bytes(&receipt).map_err(|_| {
        ContentError::Internal("failed to encode content receipt header".to_string())
    })?;
    STANDARD_NO_PAD
        .encode(encoded)
        .parse()
        .map_err(|_| ContentError::Internal("failed to encode content receipt header".to_string()))
}

fn mime_for_path(
    manifest: &ContentBundleManifest,
    entry: &iroha_data_model::content::ContentFileEntry,
) -> String {
    if let Some(mime) = manifest.mime_overrides.get(&entry.path) {
        return mime.clone();
    }

    let default = "application/octet-stream".to_string();
    let Some(ext) = entry.path.rsplit('.').next() else {
        return default;
    };
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8".to_string(),
        "css" => "text/css; charset=utf-8".to_string(),
        "js" => "application/javascript".to_string(),
        "json" => "application/json".to_string(),
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "txt" => "text/plain; charset=utf-8".to_string(),
        "wasm" => "application/wasm".to_string(),
        "ico" => "image/x-icon".to_string(),
        "gif" => "image/gif".to_string(),
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use base64::Engine;
    use iroha_config::parameters::actual::ContentPow;
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::World};
    use iroha_crypto::{KeyPair, Signature};
    use iroha_data_model::{
        Registrable,
        account::Account,
        content::{ContentCachePolicy, ContentDaReceipt, ContentFileEntry},
        da::{
            prelude::{BlobClass, DaStripeLayout},
            types::RetentionPolicy,
        },
        domain::Domain,
        nexus::{DataSpaceId, LaneId},
        role::RoleId,
    };

    fn sample_manifest() -> ContentBundleManifest {
        ContentBundleManifest {
            bundle_id: Hash::new(b"bundle"),
            index_hash: [0u8; 32],
            dataspace: DataSpaceId::GLOBAL,
            lane: LaneId::SINGLE,
            blob_class: BlobClass::GovernanceArtifact,
            retention: RetentionPolicy::default(),
            cache: ContentCachePolicy {
                max_age_seconds: 60,
                immutable: false,
            },
            auth: ContentAuthMode::Public,
            stripe_layout: DaStripeLayout::default(),
            mime_overrides: BTreeMap::new(),
        }
    }

    fn minimal_state_with_account(
        account_id: &AccountId,
        uaid: Option<iroha_data_model::nexus::UniversalAccountId>,
    ) -> std::sync::Arc<iroha_core::state::State> {
        let domain_id: iroha_data_model::domain::DomainId = "wonderland".parse().expect("domain");
        let domain = Domain::new(domain_id.clone()).build(account_id);
        let account = Account::new(account_id.to_account_id(domain_id))
            .with_uaid(uaid)
            .build(account_id);
        std::sync::Arc::new(iroha_core::state::State::new_for_testing(
            World::with([domain], [account], []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ))
    }

    fn signed_headers(
        account: &AccountId,
        key_pair: &KeyPair,
        method: &Method,
        uri: &Uri,
    ) -> HeaderMap {
        let message = crate::canonical_request_message(method, uri, &[]);
        let signature = Signature::new(key_pair.private_key(), &message);
        let mut headers = HeaderMap::new();
        headers.insert(
            crate::HEADER_ACCOUNT,
            account.to_string().parse().expect("account header"),
        );
        headers.insert(
            crate::HEADER_SIGNATURE,
            crate::signature_header_value(&signature)
                .parse()
                .expect("signature header"),
        );
        headers
    }

    #[test]
    fn bundle_expiry_is_exclusive() {
        assert!(!is_bundle_expired(4, Some(5)));
        assert!(is_bundle_expired(5, Some(5)));
        assert!(is_bundle_expired(6, Some(5)));
        assert!(!is_bundle_expired(0, None));
    }

    #[test]
    fn range_parsing_handles_full_and_partial() {
        let total_len = 11;
        let full = apply_range(total_len, None).expect("full range");
        assert_eq!(full.status, StatusCode::OK);
        assert_eq!(full.content_length, total_len);
        assert_eq!(full.start, 0);
        assert_eq!(full.end, 10);

        let partial =
            apply_range(total_len, Some(&HeaderValue::from_static("bytes=0-3"))).expect("range");
        assert_eq!(partial.status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(partial.content_range.as_deref(), Some("bytes 0-3/11"));
        assert_eq!(partial.content_length, 4);
        assert_eq!(partial.start, 0);
        assert_eq!(partial.end, 3);

        let overshoot =
            apply_range(total_len, Some(&HeaderValue::from_static("bytes=0-99"))).expect("range");
        assert_eq!(overshoot.status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(overshoot.content_range.as_deref(), Some("bytes 0-10/11"));
        assert_eq!(overshoot.content_length, 11);
        assert_eq!(overshoot.start, 0);
        assert_eq!(overshoot.end, 10);
    }

    #[test]
    fn range_header_on_empty_body_is_unsatisfiable() {
        let err = apply_range(0, Some(&HeaderValue::from_static("bytes=0-0")))
            .expect_err("range should be unsatisfiable");
        assert!(matches!(
            err,
            ContentError::RangeNotSatisfiable { total_len: 0 }
        ));
    }

    #[test]
    fn range_not_satisfiable_sets_content_range_header() {
        let response = ContentError::RangeNotSatisfiable { total_len: 12 }.into_response();
        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        let header_value = response
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok());
        assert_eq!(header_value, Some("bytes */12"));
    }

    #[test]
    fn assemble_range_from_chunks_slices_across_boundaries() {
        let chunk_hashes = [[0x01; 32], [0x02; 32], [0x03; 32]];
        let mut chunks: BTreeMap<[u8; 32], Vec<u8>> = BTreeMap::new();
        chunks.insert([0x01; 32], b"abcd".to_vec());
        chunks.insert([0x02; 32], b"efgh".to_vec());
        chunks.insert([0x03; 32], b"ijkl".to_vec());

        let body = assemble_range_from_chunks(4, &chunk_hashes, 2, 10, |hash| {
            chunks.get(hash).map(Vec::as_slice)
        })
        .expect("assemble range");

        assert_eq!(body, b"cdefghij");
    }

    #[test]
    fn mime_override_prefers_manifest() {
        let mut manifest = sample_manifest();
        manifest
            .mime_overrides
            .insert("index.html".to_string(), "text/custom".to_string());
        let entry = ContentFileEntry {
            path: "index.html".into(),
            offset: 0,
            length: 0,
            file_hash: [0; 32],
        };
        assert_eq!(mime_for_path(&manifest, &entry), "text/custom");

        let css_entry = ContentFileEntry {
            path: "styles/main.css".into(),
            offset: 0,
            length: 0,
            file_hash: [0; 32],
        };
        assert_eq!(
            mime_for_path(&manifest, &css_entry),
            "text/css; charset=utf-8"
        );
    }

    #[test]
    fn parse_bundle_id_rejects_bad_input() {
        assert!(matches!(
            parse_bundle_id("abcd"),
            Err(ContentError::BadRequest(_))
        ));
    }

    #[test]
    fn receipt_header_encodes_receipt() {
        let mut manifest = sample_manifest();
        manifest.stripe_layout.total_stripes = 1;
        let entry = ContentFileEntry {
            path: "index.html".into(),
            offset: 0,
            length: 4,
            file_hash: [5; 32],
        };
        let creator =
            AccountId::parse_encoded("6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw")
                .map(|parsed| parsed.into_account_id())
                .expect("valid account id");
        let bundle = ContentBundleRecord {
            bundle_id: Hash::new(b"bundle"),
            manifest,
            total_bytes: 4,
            chunk_size: 2,
            chunk_hashes: vec![[1; 32], [2; 32]],
            chunk_root: [9; 32],
            stripe_layout: DaStripeLayout::default(),
            pdp_commitment: None,
            files: vec![entry.clone()],
            created_by: creator,
            created_height: 1,
            expires_at_height: None,
        };
        let range_state = RangeSpec {
            status: StatusCode::OK,
            content_length: 4,
            content_range: None,
            start: 0,
            end: 3,
        };
        let header = encode_receipt_header(&bundle, &entry, &range_state).expect("receipt header");
        let decoded = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(header.as_bytes())
            .unwrap();
        let receipt: ContentDaReceipt =
            norito::decode_from_bytes(&decoded).expect("decode receipt");
        assert_eq!(receipt.served_bytes, 4);
        assert_eq!(receipt.range.unwrap().end, 3);
        assert_eq!(receipt.chunk_root.as_bytes(), &bundle.chunk_root);
    }

    #[test]
    fn rate_key_uses_headers_and_remote() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-token", HeaderValue::from_static("token-1"));
        let key = content_rate_key(&headers, None, "bundle", true);
        assert_eq!(key, "token-1");

        let key = content_rate_key(
            &HeaderMap::new(),
            Some("127.0.0.1".parse().unwrap()),
            "bundle",
            false,
        );
        assert_eq!(key, "127.0.0.1");
    }

    #[test]
    fn enforce_pow_requires_header() {
        let pow = ContentPow {
            difficulty_bits: 1,
            header_name: "x-pow".to_string(),
        };
        let headers = HeaderMap::new();
        let bundle = Hash::new(b"bundle");
        let err = enforce_pow(&pow, &headers, &bundle, "index.html").expect_err("pow missing");
        matches!(err, ContentError::Unauthorized(_));
    }

    #[test]
    fn role_gate_requires_signed_headers() {
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(key_pair.public_key().clone());
        let state = minimal_state_with_account(&account_id, None);
        let mut manifest = sample_manifest();
        manifest.auth =
            ContentAuthMode::RoleGate(RoleId::new("auditor".parse().expect("role name")));
        let headers = HeaderMap::new();
        let method = Method::GET;
        let uri: Uri = "/v2/content/abc/index.html".parse().expect("uri");

        let err = enforce_auth(&manifest, &state, &headers, &method, &uri)
            .expect_err("signature required");
        assert!(matches!(err, ContentError::Unauthorized(_)));
    }

    #[test]
    fn role_gate_rejects_missing_role() {
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(key_pair.public_key().clone());
        let state = minimal_state_with_account(&account_id, None);
        let mut manifest = sample_manifest();
        manifest.auth =
            ContentAuthMode::RoleGate(RoleId::new("auditor".parse().expect("role name")));
        let method = Method::GET;
        let uri: Uri = "/v2/content/abc/index.html".parse().expect("uri");
        let headers = signed_headers(&account_id, &key_pair, &method, &uri);

        let err =
            enforce_auth(&manifest, &state, &headers, &method, &uri).expect_err("missing role");
        assert!(matches!(err, ContentError::Forbidden(_)));
    }

    #[test]
    fn sponsor_accepts_matching_uaid() {
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(key_pair.public_key().clone());
        let uaid = iroha_data_model::nexus::UniversalAccountId::from_hash(Hash::new(b"uaid"));
        let state = minimal_state_with_account(&account_id, Some(uaid));
        let mut manifest = sample_manifest();
        manifest.auth = ContentAuthMode::Sponsor(uaid);
        let method = Method::GET;
        let uri: Uri = "/v2/content/abc/index.html".parse().expect("uri");
        let headers = signed_headers(&account_id, &key_pair, &method, &uri);

        enforce_auth(&manifest, &state, &headers, &method, &uri).expect("authorized");
    }

    #[test]
    fn sponsor_rejects_mismatched_uaid() {
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(key_pair.public_key().clone());
        let state = minimal_state_with_account(
            &account_id,
            Some(iroha_data_model::nexus::UniversalAccountId::from_hash(
                Hash::new(b"uaid"),
            )),
        );
        let mut manifest = sample_manifest();
        manifest.auth = ContentAuthMode::Sponsor(
            iroha_data_model::nexus::UniversalAccountId::from_hash(Hash::new(b"other-uaid")),
        );
        let method = Method::GET;
        let uri: Uri = "/v2/content/abc/index.html".parse().expect("uri");
        let headers = signed_headers(&account_id, &key_pair, &method, &uri);

        let err =
            enforce_auth(&manifest, &state, &headers, &method, &uri).expect_err("uaid mismatch");
        assert!(matches!(err, ContentError::Forbidden(_)));
    }

    #[test]
    fn content_bundle_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ContentBundleRecord>();
    }
}
