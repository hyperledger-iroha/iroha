use std::{
    str::FromStr,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::ConnectInfo,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use blake3::Hasher;
use hex::ToHex;
use iroha_core::state::{StateReadOnly, StateReadOnlyWithTransactions, WorldReadOnly};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    content::{
        ContentAuthMode, ContentBundleManifest, ContentBundleRecord, ContentDaReceipt, ContentRange,
    },
    da::types::BlobDigest,
    nexus::UniversalAccountId,
};
use iroha_logger::{error, info};
use norito::{codec::Encode, derive::JsonDeserialize};

use crate::{NoritoQuery, SharedAppState, limits};

#[derive(Debug)]
pub enum ContentError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound,
    RateLimited,
    Internal(String),
    RangeNotSatisfiable,
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
            Self::RangeNotSatisfiable => StatusCode::RANGE_NOT_SATISFIABLE.into_response(),
        }
    }
}

#[derive(Default, JsonDeserialize)]
pub struct ContentQuery {
    /// Optional UAID literal (`uaid:<hex>` or raw 64-hex).
    #[norito(default)]
    pub uaid: Option<String>,
    /// Optional account identifier for role-gated bundles.
    #[norito(default)]
    pub account: Option<String>,
}

/// GET /v1/content/{bundle}/{path...}
#[allow(clippy::too_many_lines)]
pub async fn handle_get_content(
    axum::extract::State(app): axum::extract::State<SharedAppState>,
    axum::extract::Path((bundle_hex, path)): axum::extract::Path<(String, String)>,
    NoritoQuery(query): NoritoQuery<ContentQuery>,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
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

        let (bundle, entry, range_state) = {
            let state = app.state.view();
            let current_height = state.height() as u64;

            let Some(bundle) =
                mv::storage::StorageReadOnly::get(state.world().content_bundles(), &bundle_id)
                    .cloned()
            else {
                outcome_hint = Some("not_found");
                return Err(ContentError::NotFound);
            };

            if let Some(expiry) = bundle.expires_at_height {
                if current_height > expiry {
                    outcome_hint = Some("not_found");
                    return Err(ContentError::NotFound);
                }
            }

            let Some(entry) = bundle.files.iter().find(|f| f.path == path).cloned() else {
                outcome_hint = Some("not_found");
                return Err(ContentError::NotFound);
            };

            if let Err(err) = enforce_auth(&bundle.manifest, &query, &state) {
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

            let assembled = match assemble_file(&state, &bundle, &entry) {
                Ok(file) => file,
                Err(err) => {
                    error!(
                        ?err,
                        bundle = %bundle_id,
                        path,
                        "failed to assemble content file"
                    );
                    outcome_hint = Some(err.outcome());
                    return Err(ContentError::Internal(err.message().to_string()));
                }
            };

            let range_state = apply_range(&assembled.body, headers.get(header::RANGE))?;

            (bundle, entry, range_state)
        };
        if !limits::allow_cost_conditionally(
            &app.content_egress_limiter,
            &key,
            range_state.content_length,
            true,
        )
        .await
        {
            outcome_hint = Some("rate_limited");
            return Err(ContentError::RateLimited);
        }

        let receipt_header = encode_receipt_header(&bundle, &entry, &range_state).ok();
        let status = range_state.status;
        let content_length = range_state.content_length;
        let content_range_header = range_state.content_range.clone();
        let body = range_state.body;

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
            status = ?range_state.status,
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
        Err(ContentError::RangeNotSatisfiable) => "range_invalid",
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
    query: &ContentQuery,
    state: &impl StateReadOnlyWithTransactions,
) -> Result<(), ContentError> {
    match &manifest.auth {
        ContentAuthMode::Public => Ok(()),
        ContentAuthMode::RoleGate(role) => {
            let account_raw = query
                .account
                .as_deref()
                .ok_or_else(|| ContentError::Unauthorized("account is required".to_string()))?;
            let account = AccountId::from_str(account_raw)
                .map_err(|_| ContentError::BadRequest("invalid account id".to_string()))?;
            let has_role = state
                .world()
                .account_roles_iter(&account)
                .any(|r| r == role);
            if has_role {
                Ok(())
            } else {
                Err(ContentError::Forbidden("role required".to_string()))
            }
        }
        ContentAuthMode::Sponsor(expected) => {
            let uaid_literal = query
                .uaid
                .as_deref()
                .ok_or_else(|| ContentError::Unauthorized("uaid is required".to_string()))?;
            let uaid = parse_uaid_literal(uaid_literal)?;
            if &uaid == expected {
                Ok(())
            } else {
                Err(ContentError::Forbidden("uaid mismatch".to_string()))
            }
        }
    }
}

fn parse_uaid_literal(raw: &str) -> Result<UniversalAccountId, ContentError> {
    let trimmed = raw.trim();
    let literal = trimmed.strip_prefix("uaid:").unwrap_or(trimmed);
    let hash = Hash::from_str(literal)
        .map_err(|_| ContentError::BadRequest("invalid uaid".to_string()))?;
    Ok(UniversalAccountId::from_hash(hash))
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

fn assemble_file(
    state: &impl StateReadOnlyWithTransactions,
    bundle: &ContentBundleRecord,
    entry: &iroha_data_model::content::ContentFileEntry,
) -> Result<AssembledFile, AssembleError> {
    let chunk_size = u64::from(bundle.chunk_size);
    let start = entry.offset;
    let end = entry
        .offset
        .checked_add(entry.length)
        .ok_or(AssembleError::Overflow)?;
    let start_chunk = start / chunk_size;
    let end_chunk = end.saturating_sub(1) / chunk_size;

    let mut out = Vec::with_capacity(entry.length as usize);
    for idx in start_chunk..=end_chunk {
        let hash = *bundle
            .chunk_hashes
            .get(idx as usize)
            .ok_or(AssembleError::MissingChunk)?;
        let chunk = mv::storage::StorageReadOnly::get(state.world().content_chunks(), &hash)
            .ok_or(AssembleError::MissingChunk)?;
        let chunk_start = idx.checked_mul(chunk_size).ok_or(AssembleError::Overflow)?;
        let slice_start = start.saturating_sub(chunk_start) as usize;
        let slice_end = (end.min(chunk_start + chunk.data.len() as u64) - chunk_start) as usize;
        if slice_start > slice_end || slice_end > chunk.data.len() {
            return Err(AssembleError::SliceBounds);
        }
        out.extend_from_slice(&chunk.data[slice_start..slice_end]);
    }
    Ok(AssembledFile {
        body: out,
        chunk_indices: (start_chunk..=end_chunk).map(|idx| idx as u32).collect(),
    })
}

struct RangeState {
    body: Vec<u8>,
    status: StatusCode,
    content_length: u64,
    content_range: Option<String>,
    start: u64,
    end: u64,
}

struct AssembledFile {
    body: Vec<u8>,
    chunk_indices: Vec<u32>,
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
    body: &[u8],
    range_header: Option<&HeaderValue>,
) -> Result<RangeState, ContentError> {
    let total_len = body.len() as u64;
    if total_len == 0 {
        return Ok(RangeState {
            body: body.to_vec(),
            status: StatusCode::OK,
            content_length: 0,
            content_range: None,
            start: 0,
            end: 0,
        });
    }
    let Some(raw) = range_header else {
        return Ok(RangeState {
            body: body.to_vec(),
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
            .ok_or(ContentError::RangeNotSatisfiable)?;
        (start, end)
    } else {
        let start: u64 = start_str
            .parse()
            .map_err(|_| ContentError::BadRequest("invalid range start".to_string()))?;
        let end: u64 = if end_str.is_empty() {
            total_len
                .checked_sub(1)
                .ok_or(ContentError::RangeNotSatisfiable)?
        } else {
            end_str
                .parse()
                .map_err(|_| ContentError::BadRequest("invalid range end".to_string()))?
        };
        (start, end)
    };

    if start >= total_len {
        return Err(ContentError::RangeNotSatisfiable);
    }

    if end >= total_len {
        end = total_len
            .checked_sub(1)
            .ok_or(ContentError::RangeNotSatisfiable)?;
    }

    if start > end {
        return Err(ContentError::RangeNotSatisfiable);
    }

    let slice = &body[start as usize..=end as usize];
    Ok(RangeState {
        body: slice.to_vec(),
        status: StatusCode::PARTIAL_CONTENT,
        content_length: slice.len() as u64,
        content_range: Some(format!("bytes {start}-{end}/{total_len}")),
        start,
        end,
    })
}

fn encode_receipt_header(
    bundle: &ContentBundleRecord,
    entry: &iroha_data_model::content::ContentFileEntry,
    range_state: &RangeState,
) -> Result<HeaderValue, ContentError> {
    let served_range = ContentRange {
        start: range_state.start,
        end: range_state.end,
    };
    let receipt = ContentDaReceipt {
        bundle_id: bundle.bundle_id,
        path: entry.path.clone(),
        file_hash: entry.file_hash,
        served_bytes: range_state.content_length,
        range: Some(served_range),
        chunk_root: BlobDigest::new(bundle.chunk_root),
        stripe_layout: bundle.stripe_layout,
        pdp_commitment: bundle.pdp_commitment.clone(),
        served_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let encoded = Encode::encode(&receipt);
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

    use base64::Engine;
    use iroha_config::parameters::actual::ContentPow;
    use iroha_data_model::{
        content::{ContentCachePolicy, ContentDaReceipt, ContentFileEntry},
        da::{
            prelude::{BlobClass, DaStripeLayout},
            types::RetentionPolicy,
        },
        nexus::{DataSpaceId, LaneId},
    };
    use norito::codec::Decode;

    use super::*;

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

    #[test]
    fn range_parsing_handles_full_and_partial() {
        let body = b"hello world".to_vec();
        let full = apply_range(&body, None).expect("full range");
        assert_eq!(full.status, StatusCode::OK);
        assert_eq!(full.content_length, body.len() as u64);
        assert_eq!(full.body, body);

        let partial =
            apply_range(&full.body, Some(&HeaderValue::from_static("bytes=0-3"))).expect("range");
        assert_eq!(partial.status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(partial.content_range.as_deref(), Some("bytes 0-3/11"));
        assert_eq!(partial.body, b"hell");

        let overshoot =
            apply_range(&full.body, Some(&HeaderValue::from_static("bytes=0-99"))).expect("range");
        assert_eq!(overshoot.status, StatusCode::PARTIAL_CONTENT);
        assert_eq!(overshoot.content_range.as_deref(), Some("bytes 0-10/11"));
        assert_eq!(overshoot.body, b"hello world");
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
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                .parse()
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
        let range_state = RangeState {
            body: Vec::new(),
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
            Decode::decode(&mut decoded.as_slice()).expect("decode receipt");
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
    fn content_bundle_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ContentBundleRecord>();
    }
}
