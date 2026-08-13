//! Exact, no-retry transport for native Bootle/Lantern blind issuance.
//!
//! This deliberately does not reuse the general Torii request pipeline. Issuance
//! credentials must never cross redirects, proxies, transparent decompression,
//! retry middleware, or generic request observers.
use std::{fmt, hint::black_box, io::Read as _, time::Duration};
use base64::{Engine as _, encoded_len, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{
    StatusCode, Url,
    blocking::{Client, Response},
    header::{
        ACCEPT, ACCEPT_ENCODING, AUTHORIZATION, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH,
        CONTENT_TYPE, HeaderMap, HeaderValue, PRAGMA, RETRY_AFTER, WWW_AUTHENTICATE,
    },
    redirect::Policy,
};
use thiserror::Error;
use iroha_torii_shared::ErrorEnvelope;
/// Canonical authorization endpoint.
pub const BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1: &str =
    "/v1/privacy/bootle-lantern/issuance/authorize";
/// Canonical one-shot issuance endpoint.
pub const BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1: &str = "/v1/privacy/bootle-lantern/issuance/issue";
/// Sole request and successful-response media type.
pub const BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1: &str = "application/x-norito";
/// Maximum decoded opaque bearer credential length accepted by Torii.
pub const BOOTLE_LANTERN_ISSUANCE_AUTHENTICATION_MAX_BYTES_V1: usize = 4_096;
/// Exact `ILA1` authorization response length.
pub const BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1: usize = 320;
/// Exact `ILA1 || ILQ1` issue request length.
pub const BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1: usize = 71_896;
/// Exact `ILR1` issuance response length.
pub const BOOTLE_LANTERN_ISSUANCE_ISSUE_RESPONSE_BYTES_V1: usize = 3_176;
/// Maximum accepted structured error response length.
pub const BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1: usize = 512;
const JSON_MEDIA_TYPE_V1: &str = "application/json";
const AUTHORIZATION_MAGIC_V1: &[u8; 4] = b"ILA1";
const BLIND_REQUEST_MAGIC_V1: &[u8; 4] = b"ILQ1";
const RESPONSE_MAGIC_V1: &[u8; 4] = b"ILR1";
const WWW_AUTHENTICATE_VALUE_V1: &[u8] = b"Bearer realm=\"iroha-bootle-lantern-issuance\"";
const DEFAULT_REQUEST_TIMEOUT_V1: Duration = Duration::from_secs(15);
/// Opaque issuer credential erased when dropped.
pub struct BootleLanternIssuanceCredentialV1 {
    bytes: Vec<u8>,
}
impl BootleLanternIssuanceCredentialV1 {
    /// Copy and validate an opaque bearer credential.
    ///
    /// # Errors
    ///
    /// Returns [`BootleLanternIssuanceClientErrorV1::InvalidCredential`] when
    /// the credential is empty or exceeds the protocol limit.
    pub fn from_opaque_bytes(bytes: &[u8]) -> Result<Self, BootleLanternIssuanceClientErrorV1> {
        validate_credential_length_v1(bytes.len())?;
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }
    /// Decode one canonical, unpadded base64url credential without a `Bearer` prefix.
    ///
    /// # Errors
    ///
    /// Returns [`BootleLanternIssuanceClientErrorV1::InvalidCredential`] when
    /// the input is empty, oversized, malformed, padded, or non-canonical.
    pub fn from_canonical_base64_url(
        encoded: &str,
    ) -> Result<Self, BootleLanternIssuanceClientErrorV1> {
        let encoded_max = encoded_len(BOOTLE_LANTERN_ISSUANCE_AUTHENTICATION_MAX_BYTES_V1, false)
            .expect("fixed credential length has a base64url representation");
        if encoded.is_empty()
            || encoded.len() > encoded_max
            || encoded
                .bytes()
                .any(|byte| byte == b'=' || byte.is_ascii_whitespace())
        {
            return Err(BootleLanternIssuanceClientErrorV1::InvalidCredential);
        }
        let mut bytes = vec![0_u8; BOOTLE_LANTERN_ISSUANCE_AUTHENTICATION_MAX_BYTES_V1];
        let Ok(written) = URL_SAFE_NO_PAD.decode_slice(encoded, &mut bytes) else {
            bytes.fill(0);
            black_box(bytes.as_slice());
            return Err(BootleLanternIssuanceClientErrorV1::InvalidCredential);
        };
        bytes.truncate(written);
        let result = (|| {
            validate_credential_length_v1(bytes.len())?;
            if !canonical_base64_url_matches_v1(encoded, &bytes) {
                return Err(BootleLanternIssuanceClientErrorV1::InvalidCredential);
            }
            Ok(Self {
                bytes: std::mem::take(&mut bytes),
            })
        })();
        bytes.fill(0);
        black_box(bytes.as_slice());
        result
    }
    fn authorization_header_value_v1(
        &self,
    ) -> Result<HeaderValue, BootleLanternIssuanceClientErrorV1> {
        let encoded_bytes = encoded_len(self.bytes.len(), false)
            .expect("bounded credential length has a base64url representation");
        let mut value = vec![0_u8; b"Bearer ".len().saturating_add(encoded_bytes)];
        value[..b"Bearer ".len()].copy_from_slice(b"Bearer ");
        let result = URL_SAFE_NO_PAD
            .encode_slice(&self.bytes, &mut value[b"Bearer ".len()..])
            .map_err(|_| BootleLanternIssuanceClientErrorV1::InvalidCredential)
            .and_then(|written| {
                value.truncate(b"Bearer ".len().saturating_add(written));
                let mut header = HeaderValue::from_bytes(&value)
                    .map_err(|_| BootleLanternIssuanceClientErrorV1::InvalidCredential)?;
                header.set_sensitive(true);
                Ok(header)
            });
        value.fill(0);
        black_box(value.as_slice());
        result
    }
}
impl fmt::Debug for BootleLanternIssuanceCredentialV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BootleLanternIssuanceCredentialV1([REDACTED])")
    }
}
impl Drop for BootleLanternIssuanceCredentialV1 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        black_box(self.bytes.as_slice());
    }
}
/// Exact transport failures for Bootle/Lantern blind issuance.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BootleLanternIssuanceClientErrorV1 {
    /// The Torii origin is not an origin-only absolute HTTPS URL.
    #[error("Bootle/Lantern issuance requires an origin-only absolute HTTPS URL")]
    InvalidOrigin,
    /// The opaque credential is empty, oversized, or not canonical base64url.
    #[error("invalid Bootle/Lantern issuance credential")]
    InvalidCredential,
    /// The request timeout is zero.
    #[error("Bootle/Lantern issuance timeout must be non-zero")]
    InvalidTimeout,
    /// The dedicated hardened HTTP client could not be built.
    #[error("failed to build Bootle/Lantern issuance transport")]
    TransportBuild,
    /// The exact request could not be constructed.
    #[error("failed to build Bootle/Lantern issuance request")]
    RequestBuild,
    /// The single network attempt failed.
    #[error("Bootle/Lantern issuance network attempt failed")]
    Network,
    /// The issue payload was not exactly `ILA1 || ILQ1` sized.
    #[error("Bootle/Lantern issue request has an invalid length")]
    InvalidIssueRequestLength,
    /// The exact-size issue payload did not contain canonical `ILA1 || ILQ1` magics.
    #[error("Bootle/Lantern issue request has an invalid wire magic")]
    InvalidIssueRequestMagic,
    /// Torii returned a canonical structured issuance error.
    #[error("Bootle/Lantern issuance returned HTTP {status}: {code}")]
    Http {
        /// HTTP response status.
        status: u16,
        /// Stable first-release Torii error code.
        code: String,
        /// Exact server retry hint. Present only for HTTP 429.
        retry_after_seconds: Option<u64>,
    },
    /// A non-success response was not a canonical issuance error.
    #[error("Bootle/Lantern issuance returned an invalid error response")]
    InvalidErrorResponse,
    /// The successful response did not contain exactly the canonical media type.
    #[error("Bootle/Lantern issuance response has an invalid content type")]
    InvalidContentType,
    /// The successful response declared a content encoding.
    #[error("Bootle/Lantern issuance response must use identity encoding")]
    ContentEncoding,
    /// The successful response declared an invalid or contradictory content length.
    #[error("Bootle/Lantern issuance response has an invalid content length")]
    InvalidContentLength,
    /// A successful response carried an authentication challenge reserved for HTTP 401.
    #[error("Bootle/Lantern issuance response contains an unexpected authentication challenge")]
    UnexpectedAuthenticationChallenge,
    /// The successful response body was truncated or oversized.
    #[error("Bootle/Lantern issuance response has an invalid body length")]
    InvalidResponseLength,
    /// The successful response did not begin with its canonical `ILA1` or `ILR1` magic.
    #[error("Bootle/Lantern issuance response has an invalid wire magic")]
    InvalidResponseMagic,
}
/// Dedicated synchronous client for the two first-release issuance routes.
pub struct BootleLanternIssuanceClientV1 {
    origin: Url,
    transport: Client,
}
impl fmt::Debug for BootleLanternIssuanceClientV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BootleLanternIssuanceClientV1")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}
impl BootleLanternIssuanceClientV1 {
    /// Build a dedicated client with a 15-second per-request timeout.
    ///
    /// # Errors
    ///
    /// Returns an error when `origin` is not an origin-only absolute HTTPS URL
    /// or the hardened HTTP transport cannot be constructed.
    pub fn new(origin: Url) -> Result<Self, BootleLanternIssuanceClientErrorV1> {
        Self::with_timeout(origin, DEFAULT_REQUEST_TIMEOUT_V1)
    }
    /// Build a dedicated client with an exact non-zero per-request timeout.
    ///
    /// # Errors
    ///
    /// Returns an error when `origin` is not an origin-only absolute HTTPS URL,
    /// `timeout` is zero, or the hardened HTTP transport cannot be constructed.
    pub fn with_timeout(
        origin: Url,
        timeout: Duration,
    ) -> Result<Self, BootleLanternIssuanceClientErrorV1> {
        validate_origin_v1(&origin)?;
        if timeout.is_zero() {
            return Err(BootleLanternIssuanceClientErrorV1::InvalidTimeout);
        }
        let transport = Client::builder()
            .redirect(Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_zstd()
            .no_deflate()
            .timeout(timeout)
            .build()
            .map_err(|_| BootleLanternIssuanceClientErrorV1::TransportBuild)?;
        Ok(Self { origin, transport })
    }
    /// Submit exactly one empty authorization request and return the 320-byte `ILA1` body.
    ///
    /// # Errors
    ///
    /// Returns an error when request construction or the single network attempt
    /// fails, or when Torii returns a non-canonical response or protocol error.
    pub fn authorize(
        &self,
        credential: &BootleLanternIssuanceCredentialV1,
    ) -> Result<Vec<u8>, BootleLanternIssuanceClientErrorV1> {
        self.execute_exact_v1(
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1,
            credential,
            Vec::new(),
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1,
        )
    }
    /// Submit exactly one `ILA1 || ILQ1` request and return the 3,176-byte `ILR1` body.
    ///
    /// # Errors
    ///
    /// Returns an error when the request has the wrong length or magic, request
    /// construction or the single network attempt fails, or Torii returns a
    /// non-canonical response or protocol error.
    pub fn issue(
        &self,
        credential: &BootleLanternIssuanceCredentialV1,
        canonical_request: &[u8],
    ) -> Result<Vec<u8>, BootleLanternIssuanceClientErrorV1> {
        if canonical_request.len() != BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1 {
            return Err(BootleLanternIssuanceClientErrorV1::InvalidIssueRequestLength);
        }
        if !canonical_request.starts_with(AUTHORIZATION_MAGIC_V1)
            || canonical_request.get(
                BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1
                    ..BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1
                        + BLIND_REQUEST_MAGIC_V1.len(),
            ) != Some(BLIND_REQUEST_MAGIC_V1)
        {
            return Err(BootleLanternIssuanceClientErrorV1::InvalidIssueRequestMagic);
        }
        self.execute_exact_v1(
            BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1,
            credential,
            canonical_request.to_vec(),
            BOOTLE_LANTERN_ISSUANCE_ISSUE_RESPONSE_BYTES_V1,
        )
    }
    fn execute_exact_v1(
        &self,
        path: &'static str,
        credential: &BootleLanternIssuanceCredentialV1,
        body: Vec<u8>,
        expected_response_bytes: usize,
    ) -> Result<Vec<u8>, BootleLanternIssuanceClientErrorV1> {
        self.execute_exact_with_v1(
            path,
            credential,
            body,
            expected_response_bytes,
            |request, expected_bytes| {
                let response = self
                    .transport
                    .execute(request)
                    .map_err(|_| BootleLanternIssuanceClientErrorV1::Network)?;
                collect_network_response_v1(response, expected_bytes)
            },
        )
    }
    fn execute_exact_with_v1<F>(
        &self,
        path: &'static str,
        credential: &BootleLanternIssuanceCredentialV1,
        body: Vec<u8>,
        expected_response_bytes: usize,
        send_once: F,
    ) -> Result<Vec<u8>, BootleLanternIssuanceClientErrorV1>
    where
        F: FnOnce(
            reqwest::blocking::Request,
            usize,
        ) -> Result<RawIssuanceResponseV1, BootleLanternIssuanceClientErrorV1>,
    {
        let request = self.build_request_v1(path, credential, body)?;
        let response = send_once(request, expected_response_bytes)?;
        validate_raw_response_v1(response, expected_response_bytes)
    }
    fn build_request_v1(
        &self,
        path: &'static str,
        credential: &BootleLanternIssuanceCredentialV1,
        body: Vec<u8>,
    ) -> Result<reqwest::blocking::Request, BootleLanternIssuanceClientErrorV1> {
        let mut target = self.origin.clone();
        target.set_path(path);
        let request = self
            .transport
            .post(target)
            .header(AUTHORIZATION, credential.authorization_header_value_v1()?)
            .header(CONTENT_TYPE, BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1)
            .header(ACCEPT, BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1)
            .header(ACCEPT_ENCODING, "identity")
            .header(CACHE_CONTROL, "no-store")
            .header(PRAGMA, "no-cache")
            .body(body)
            .build()
            .map_err(|_| BootleLanternIssuanceClientErrorV1::RequestBuild)?;
        Ok(request)
    }
}
struct RawIssuanceResponseV1 {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}
fn validate_origin_v1(origin: &Url) -> Result<(), BootleLanternIssuanceClientErrorV1> {
    let valid_path = origin.path().is_empty() || origin.path() == "/";
    if origin.scheme() != "https"
        || origin.host_str().is_none()
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || !valid_path
    {
        return Err(BootleLanternIssuanceClientErrorV1::InvalidOrigin);
    }
    Ok(())
}
fn validate_credential_length_v1(length: usize) -> Result<(), BootleLanternIssuanceClientErrorV1> {
    if length == 0 || length > BOOTLE_LANTERN_ISSUANCE_AUTHENTICATION_MAX_BYTES_V1 {
        return Err(BootleLanternIssuanceClientErrorV1::InvalidCredential);
    }
    Ok(())
}
fn canonical_base64_url_matches_v1(encoded: &str, decoded: &[u8]) -> bool {
    let encoded_bytes = encoded_len(decoded.len(), false)
        .expect("bounded credential length has a base64url representation");
    let mut canonical = vec![0_u8; encoded_bytes];
    let matches = URL_SAFE_NO_PAD
        .encode_slice(decoded, &mut canonical)
        .ok()
        .is_some_and(|written| {
            canonical.truncate(written);
            canonical.as_slice() == encoded.as_bytes()
        });
    canonical.fill(0);
    black_box(canonical.as_slice());
    matches
}
fn collect_network_response_v1(
    mut response: Response,
    expected_bytes: usize,
) -> Result<RawIssuanceResponseV1, BootleLanternIssuanceClientErrorV1> {
    let maximum_bytes = if response.status() == StatusCode::OK {
        expected_bytes
    } else {
        BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1
    };
    let mut body = Vec::with_capacity(maximum_bytes);
    response
        .by_ref()
        .take(
            u64::try_from(maximum_bytes)
                .expect("fixed issuance response length fits u64")
                .saturating_add(1),
        )
        .read_to_end(&mut body)
        .map_err(|_| BootleLanternIssuanceClientErrorV1::Network)?;
    Ok(RawIssuanceResponseV1 {
        status: response.status(),
        headers: response.headers().clone(),
        body,
    })
}
fn validate_raw_response_v1(
    response: RawIssuanceResponseV1,
    expected_bytes: usize,
) -> Result<Vec<u8>, BootleLanternIssuanceClientErrorV1> {
    if response.status == StatusCode::OK {
        validate_success_response_metadata_v1(&response.headers, expected_bytes)?;
        if response.body.len() != expected_bytes {
            return Err(BootleLanternIssuanceClientErrorV1::InvalidResponseLength);
        }
        let expected_magic = match expected_bytes {
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1 => AUTHORIZATION_MAGIC_V1,
            BOOTLE_LANTERN_ISSUANCE_ISSUE_RESPONSE_BYTES_V1 => RESPONSE_MAGIC_V1,
            _ => return Err(BootleLanternIssuanceClientErrorV1::InvalidResponseLength),
        };
        if !response.body.starts_with(expected_magic) {
            return Err(BootleLanternIssuanceClientErrorV1::InvalidResponseMagic);
        }
        return Ok(response.body);
    }
    Err(decode_error_response_v1(
        response.status,
        &response.headers,
        &response.body,
    ))
}
fn validate_success_response_metadata_v1(
    headers: &HeaderMap,
    expected_bytes: usize,
) -> Result<(), BootleLanternIssuanceClientErrorV1> {
    if !has_exact_single_header_v1(
        headers,
        CONTENT_TYPE,
        BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1.as_bytes(),
    ) {
        return Err(BootleLanternIssuanceClientErrorV1::InvalidContentType);
    }
    if headers.get_all(CONTENT_ENCODING).iter().next().is_some() {
        return Err(BootleLanternIssuanceClientErrorV1::ContentEncoding);
    }
    if headers.get_all(WWW_AUTHENTICATE).iter().next().is_some() {
        return Err(BootleLanternIssuanceClientErrorV1::UnexpectedAuthenticationChallenge);
    }
    let mut content_lengths = headers.get_all(CONTENT_LENGTH).iter();
    if let Some(value) = content_lengths.next()
        && (content_lengths.next().is_some()
            || !canonical_content_length_v1(value.as_bytes(), expected_bytes))
    {
        return Err(BootleLanternIssuanceClientErrorV1::InvalidContentLength);
    }
    Ok(())
}
fn decode_error_response_v1(
    status: StatusCode,
    headers: &HeaderMap,
    body: &[u8],
) -> BootleLanternIssuanceClientErrorV1 {
    let Some((expected_code, expected_media_type)) = issuance_error_contract_v1(status) else {
        return BootleLanternIssuanceClientErrorV1::InvalidErrorResponse;
    };
    if body.is_empty()
        || body.len() > BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1
        || !has_exact_single_header_v1(headers, CONTENT_TYPE, expected_media_type.as_bytes())
        || headers.get_all(CONTENT_ENCODING).iter().next().is_some()
        || !has_canonical_optional_content_length_v1(headers, body.len())
        || !has_canonical_retry_after_v1(headers, status)
        || !has_canonical_www_authenticate_v1(headers, status)
    {
        return BootleLanternIssuanceClientErrorV1::InvalidErrorResponse;
    }
    let envelope = if status == StatusCode::NOT_ACCEPTABLE {
        let expected = format!("{{\"code\":\"{expected_code}\",\"message\":\"{expected_code}\"}}");
        if body != expected.as_bytes() {
            return BootleLanternIssuanceClientErrorV1::InvalidErrorResponse;
        }
        norito::json::from_slice::<ErrorEnvelope>(body).ok()
    } else {
        norito::decode_canonical::<ErrorEnvelope>(body).ok()
    };
    let Some(envelope) = envelope else {
        return BootleLanternIssuanceClientErrorV1::InvalidErrorResponse;
    };
    if envelope.code != expected_code
        || envelope.message != expected_code
        || envelope.details.is_some()
    {
        return BootleLanternIssuanceClientErrorV1::InvalidErrorResponse;
    }
    BootleLanternIssuanceClientErrorV1::Http {
        status: status.as_u16(),
        code: envelope.code,
        retry_after_seconds: (status == StatusCode::TOO_MANY_REQUESTS).then_some(1),
    }
}
fn issuance_error_contract_v1(status: StatusCode) -> Option<(&'static str, &'static str)> {
    let code = match status {
        StatusCode::BAD_REQUEST => "privacy_issuance_invalid_request",
        StatusCode::UNAUTHORIZED => "privacy_issuance_unauthorized",
        StatusCode::NOT_ACCEPTABLE => "privacy_issuance_not_acceptable",
        StatusCode::CONFLICT => "privacy_issuance_state_conflict",
        StatusCode::PAYLOAD_TOO_LARGE => "privacy_issuance_payload_too_large",
        StatusCode::UNSUPPORTED_MEDIA_TYPE => "privacy_issuance_unsupported_media_type",
        StatusCode::TOO_MANY_REQUESTS => "privacy_issuance_capacity_exhausted",
        StatusCode::SERVICE_UNAVAILABLE => "privacy_issuance_unavailable",
        _ => return None,
    };
    let media_type = if status == StatusCode::NOT_ACCEPTABLE {
        JSON_MEDIA_TYPE_V1
    } else {
        BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1
    };
    Some((code, media_type))
}
fn has_exact_single_header_v1(
    headers: &HeaderMap,
    name: reqwest::header::HeaderName,
    expected: &[u8],
) -> bool {
    let mut values = headers.get_all(name).iter();
    values
        .next()
        .filter(|_| values.next().is_none())
        .is_some_and(|value| value.as_bytes() == expected)
}
fn has_canonical_optional_content_length_v1(headers: &HeaderMap, actual_bytes: usize) -> bool {
    let mut values = headers.get_all(CONTENT_LENGTH).iter();
    let Some(value) = values.next() else {
        return true;
    };
    values.next().is_none() && canonical_content_length_v1(value.as_bytes(), actual_bytes)
}
fn has_canonical_retry_after_v1(headers: &HeaderMap, status: StatusCode) -> bool {
    let mut values = headers.get_all(RETRY_AFTER).iter();
    if status == StatusCode::TOO_MANY_REQUESTS {
        return values
            .next()
            .filter(|_| values.next().is_none())
            .is_some_and(|value| value.as_bytes() == b"1");
    }
    values.next().is_none()
}
fn has_canonical_www_authenticate_v1(headers: &HeaderMap, status: StatusCode) -> bool {
    let mut values = headers.get_all(WWW_AUTHENTICATE).iter();
    if status == StatusCode::UNAUTHORIZED {
        return values
            .next()
            .filter(|_| values.next().is_none())
            .is_some_and(|value| value.as_bytes() == WWW_AUTHENTICATE_VALUE_V1);
    }
    values.next().is_none()
}
fn canonical_content_length_v1(bytes: &[u8], expected_bytes: usize) -> bool {
    if bytes.is_empty()
        || bytes.iter().any(|byte| !byte.is_ascii_digit())
        || (bytes.len() > 1 && bytes[0] == b'0')
    {
        return false;
    }
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        == Some(expected_bytes)
}
#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use base64::Engine as _;
    use sha2::{Digest as _, Sha256};
    use super::*;
    const CLIENT_CONTRACT_FIXTURE_V1: &str =
        include_str!("../../../fixtures/privacy/bootle_lantern_issuance_client_v1.json");
    fn patterned_v1(length: usize) -> Vec<u8> {
        let mut body: Vec<u8> = (0_u8..=u8::MAX).cycle().take(length).collect();
        match length {
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1 => {
                body[..AUTHORIZATION_MAGIC_V1.len()].copy_from_slice(AUTHORIZATION_MAGIC_V1);
            }
            BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1 => {
                body[..AUTHORIZATION_MAGIC_V1.len()].copy_from_slice(AUTHORIZATION_MAGIC_V1);
                body[BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1
                    ..BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1 + 4]
                    .copy_from_slice(b"ILQ1");
            }
            BOOTLE_LANTERN_ISSUANCE_ISSUE_RESPONSE_BYTES_V1 => {
                body[..RESPONSE_MAGIC_V1.len()].copy_from_slice(RESPONSE_MAGIC_V1);
            }
            _ => {}
        }
        body
    }
    fn client_v1() -> BootleLanternIssuanceClientV1 {
        BootleLanternIssuanceClientV1::new(
            Url::parse("https://validator.example").expect("valid test URL"),
        )
        .expect("valid client")
    }
    fn credential_v1() -> BootleLanternIssuanceCredentialV1 {
        BootleLanternIssuanceCredentialV1::from_opaque_bytes(b"opaque credential")
            .expect("valid credential")
    }
    fn response_v1(length: usize) -> RawIssuanceResponseV1 {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static(BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1),
        );
        headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&length.to_string()).expect("valid length header"),
        );
        RawIssuanceResponseV1 {
            status: StatusCode::OK,
            headers,
            body: patterned_v1(length),
        }
    }
    fn error_response_v1(status: StatusCode, code: &str) -> RawIssuanceResponseV1 {
        let media_type = if status == StatusCode::NOT_ACCEPTABLE {
            JSON_MEDIA_TYPE_V1
        } else {
            BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1
        };
        let body = if status == StatusCode::NOT_ACCEPTABLE {
            format!("{{\"code\":\"{code}\",\"message\":\"{code}\"}}").into_bytes()
        } else {
            norito::encode_canonical(&ErrorEnvelope::new(code, code))
                .expect("canonical error envelope")
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(media_type).expect("valid media type"),
        );
        headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&body.len().to_string()).expect("valid length header"),
        );
        if status == StatusCode::TOO_MANY_REQUESTS {
            headers.insert(RETRY_AFTER, HeaderValue::from_static("1"));
        }
        if status == StatusCode::UNAUTHORIZED {
            headers.insert(
                WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer realm=\"iroha-bootle-lantern-issuance\""),
            );
        }
        RawIssuanceResponseV1 {
            status,
            headers,
            body,
        }
    }
    #[test]
    fn origin_is_exact_https_origin() {
        for invalid in [
            "http://validator.example",
            "ftp://validator.example",
            "https://user@validator.example",
            "https://user:secret@validator.example",
            "https://validator.example/path",
            "https://validator.example//",
            "https://validator.example?query=yes",
            "https://validator.example#fragment",
        ] {
            let error = BootleLanternIssuanceClientV1::new(
                Url::parse(invalid).expect("syntactically valid URL"),
            )
            .expect_err("non-origin HTTPS target must fail");
            assert_eq!(error, BootleLanternIssuanceClientErrorV1::InvalidOrigin);
        }
        assert_eq!(
            BootleLanternIssuanceClientV1::with_timeout(
                Url::parse("https://validator.example").expect("valid URL"),
                Duration::ZERO,
            )
            .expect_err("zero timeout must fail"),
            BootleLanternIssuanceClientErrorV1::InvalidTimeout
        );
    }
    #[test]
    fn credential_rejects_noncanonical_and_redacts_debug() {
        assert_eq!(
            BootleLanternIssuanceCredentialV1::from_opaque_bytes(&[])
                .expect_err("empty credential must fail"),
            BootleLanternIssuanceClientErrorV1::InvalidCredential
        );
        assert_eq!(
            BootleLanternIssuanceCredentialV1::from_opaque_bytes(&vec![
                0;
                BOOTLE_LANTERN_ISSUANCE_AUTHENTICATION_MAX_BYTES_V1
                    + 1
            ])
            .expect_err("oversized credential must fail"),
            BootleLanternIssuanceClientErrorV1::InvalidCredential
        );
        for invalid in ["", "Zg=", "Zg==", "Zg\n", "Zg ", "+w", "Zh"] {
            assert_eq!(
                BootleLanternIssuanceCredentialV1::from_canonical_base64_url(invalid)
                    .expect_err("noncanonical credential must fail"),
                BootleLanternIssuanceClientErrorV1::InvalidCredential,
                "credential {invalid:?}"
            );
        }
        let credential = BootleLanternIssuanceCredentialV1::from_canonical_base64_url("Zg")
            .expect("canonical credential");
        assert_eq!(
            format!("{credential:?}"),
            "BootleLanternIssuanceCredentialV1([REDACTED])"
        );
        let maximum = vec![0xA5; BOOTLE_LANTERN_ISSUANCE_AUTHENTICATION_MAX_BYTES_V1];
        let mut maximum_encoded = vec![
            0_u8;
            encoded_len(maximum.len(), false).expect(
                "fixed credential length has a base64url representation"
            )
        ];
        let written = URL_SAFE_NO_PAD
            .encode_slice(&maximum, &mut maximum_encoded)
            .expect("exact base64url output buffer");
        let maximum_encoded =
            std::str::from_utf8(&maximum_encoded[..written]).expect("base64url is valid UTF-8");
        BootleLanternIssuanceCredentialV1::from_canonical_base64_url(maximum_encoded)
            .expect("maximum-length credential must be accepted");
        let oversized_encoded = "A".repeat(maximum_encoded.len().saturating_add(1));
        assert_eq!(
            BootleLanternIssuanceCredentialV1::from_canonical_base64_url(&oversized_encoded)
                .expect_err("oversized base64url credential must fail before decoding"),
            BootleLanternIssuanceClientErrorV1::InvalidCredential
        );
    }
    #[test]
    fn shared_client_contract_fixture_binds_exact_wire_bytes() {
        let fixture: norito::json::Value =
            norito::json::from_str(CLIENT_CONTRACT_FIXTURE_V1).expect("valid client fixture JSON");
        let root = fixture.as_object().expect("client fixture object");
        assert_eq!(
            root.get("schema").and_then(norito::json::Value::as_str),
            Some("iroha.bootle_lantern.issuance_client_contract")
        );
        assert_eq!(
            root.get("version").and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            root.get("classification")
                .and_then(norito::json::Value::as_str),
            Some("public-synthetic-test-data")
        );
        let transport = root
            .get("transport")
            .and_then(norito::json::Value::as_object)
            .expect("transport fixture object");
        assert_eq!(
            transport
                .get("method")
                .and_then(norito::json::Value::as_str),
            Some("POST")
        );
        assert_eq!(
            transport
                .get("authorize_path")
                .and_then(norito::json::Value::as_str),
            Some(BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1)
        );
        assert_eq!(
            transport
                .get("issue_path")
                .and_then(norito::json::Value::as_str),
            Some(BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1)
        );
        assert_eq!(
            transport
                .get("norito_media_type")
                .and_then(norito::json::Value::as_str),
            Some(BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1)
        );
        assert_eq!(
            transport
                .get("unauthorized_www_authenticate")
                .and_then(norito::json::Value::as_str),
            std::str::from_utf8(WWW_AUTHENTICATE_VALUE_V1).ok()
        );
        let credential = root
            .get("credential")
            .and_then(norito::json::Value::as_object)
            .expect("credential fixture object");
        assert_eq!(
            credential
                .get("encoding")
                .and_then(norito::json::Value::as_str),
            Some("base64url-unpadded-canonical")
        );
        assert_eq!(
            credential
                .get("minimum_decoded_bytes")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            credential
                .get("maximum_decoded_bytes")
                .and_then(norito::json::Value::as_u64),
            Some(
                u64::try_from(BOOTLE_LANTERN_ISSUANCE_AUTHENTICATION_MAX_BYTES_V1)
                    .expect("credential bound fits u64")
            )
        );
        let examples = credential
            .get("examples")
            .and_then(norito::json::Value::as_array)
            .expect("credential examples array");
        assert_eq!(examples.len(), 3);
        for example in examples {
            let example = example.as_object().expect("credential example object");
            let decoded = hex::decode(
                example
                    .get("decoded_hex")
                    .and_then(norito::json::Value::as_str)
                    .expect("credential example hex"),
            )
            .expect("canonical credential example hex");
            let encoded = example
                .get("encoded")
                .and_then(norito::json::Value::as_str)
                .expect("credential example encoding");
            assert_eq!(URL_SAFE_NO_PAD.encode(&decoded), encoded);
            let admitted = BootleLanternIssuanceCredentialV1::from_canonical_base64_url(encoded)
                .expect("fixture credential must be admitted");
            assert_eq!(
                admitted
                    .authorization_header_value_v1()
                    .expect("fixture authorization header")
                    .to_str()
                    .expect("ASCII authorization header"),
                format!("Bearer {encoded}")
            );
        }
        let bodies = root
            .get("bodies")
            .and_then(norito::json::Value::as_object)
            .expect("body fixture object");
        assert_eq!(
            bodies.get("pattern").and_then(norito::json::Value::as_str),
            Some("byte-at-index-equals-index-modulo-256-with-canonical-wire-magics")
        );
        let assert_body = |name: &str, wire: &str, length: usize| {
            let body = bodies
                .get(name)
                .and_then(norito::json::Value::as_object)
                .expect("named body fixture object");
            assert_eq!(
                body.get("wire").and_then(norito::json::Value::as_str),
                Some(wire)
            );
            assert_eq!(
                body.get("length_bytes")
                    .and_then(norito::json::Value::as_u64),
                Some(u64::try_from(length).expect("wire length fits u64"))
            );
            assert_eq!(
                hex::encode(Sha256::digest(patterned_v1(length))),
                body.get("pattern_sha256_hex")
                    .and_then(norito::json::Value::as_str)
                    .expect("pattern digest")
            );
        };
        assert_body(
            "authorization_response",
            "ILA1",
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1,
        );
        assert_body(
            "issue_request",
            "ILA1+ILQ1",
            BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1,
        );
        assert_body(
            "issue_response",
            "ILR1",
            BOOTLE_LANTERN_ISSUANCE_ISSUE_RESPONSE_BYTES_V1,
        );
        let issue_request = bodies
            .get("issue_request")
            .and_then(norito::json::Value::as_object)
            .expect("issue request fixture object");
        let components = issue_request
            .get("component_lengths_bytes")
            .and_then(norito::json::Value::as_array)
            .expect("issue request components");
        assert_eq!(components.len(), 2);
        assert_eq!(components[0].as_u64(), Some(320));
        assert_eq!(components[1].as_u64(), Some(71_576));
        assert_eq!(
            components
                .iter()
                .filter_map(norito::json::Value::as_u64)
                .sum::<u64>(),
            u64::try_from(BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1)
                .expect("issue request bound fits u64")
        );
        let errors = root
            .get("errors")
            .and_then(norito::json::Value::as_object)
            .expect("error fixture object");
        assert_eq!(
            errors
                .get("maximum_body_bytes")
                .and_then(norito::json::Value::as_u64),
            Some(
                u64::try_from(BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1)
                    .expect("error response bound fits u64")
            )
        );
        let envelope = errors
            .get("norito_envelope")
            .and_then(norito::json::Value::as_object)
            .expect("error envelope fixture object");
        assert_eq!(
            envelope
                .get("schema_type_name")
                .and_then(norito::json::Value::as_str),
            Some("iroha_torii_shared::ErrorEnvelope")
        );
        assert_eq!(
            envelope
                .get("schema_hash_hex")
                .and_then(norito::json::Value::as_str),
            Some("793f11768076bfe270a17aeb86752cd9")
        );
        assert_eq!(
            envelope
                .get("flags_hex")
                .and_then(norito::json::Value::as_str),
            Some("02")
        );
        let responses = errors
            .get("responses")
            .and_then(norito::json::Value::as_array)
            .expect("error response fixtures");
        assert_eq!(responses.len(), 8);
        for fixture_response in responses {
            let fixture_response = fixture_response
                .as_object()
                .expect("error response fixture object");
            let status = StatusCode::from_u16(
                u16::try_from(
                    fixture_response
                        .get("status")
                        .and_then(norito::json::Value::as_u64)
                        .expect("error response status"),
                )
                .expect("fixture status fits u16"),
            )
            .expect("valid fixture status");
            let code = fixture_response
                .get("code")
                .and_then(norito::json::Value::as_str)
                .expect("fixture error code");
            assert_eq!(
                fixture_response
                    .get("www_authenticate")
                    .and_then(norito::json::Value::as_str),
                (status == StatusCode::UNAUTHORIZED)
                    .then(
                        || std::str::from_utf8(WWW_AUTHENTICATE_VALUE_V1).expect("ASCII challenge")
                    )
            );
            let response = error_response_v1(status, code);
            let expected_body = if let Some(body_hex) = fixture_response
                .get("body_hex")
                .and_then(norito::json::Value::as_str)
            {
                hex::decode(body_hex).expect("canonical error fixture hex")
            } else {
                fixture_response
                    .get("body_utf8")
                    .and_then(norito::json::Value::as_str)
                    .expect("canonical JSON error fixture")
                    .as_bytes()
                    .to_vec()
            };
            assert_eq!(response.body, expected_body);
            assert_eq!(
                validate_raw_response_v1(response, 320).expect_err("fixture is an HTTP error"),
                BootleLanternIssuanceClientErrorV1::Http {
                    status: status.as_u16(),
                    code: code.to_owned(),
                    retry_after_seconds: (status == StatusCode::TOO_MANY_REQUESTS).then_some(1),
                }
            );
        }
    }
    #[test]
    fn request_is_exact_and_send_is_invoked_once() {
        let client = client_v1();
        let credential = credential_v1();
        let calls = Cell::new(0_u8);
        let response = client
            .execute_exact_with_v1(
                BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1,
                &credential,
                Vec::new(),
                BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1,
                |request, expected| {
                    calls.set(calls.get().saturating_add(1));
                    assert_eq!(request.method(), reqwest::Method::POST);
                    assert_eq!(request.headers().len(), 6);
                    assert_eq!(
                        request.url().as_str(),
                        "https://validator.example/v1/privacy/bootle-lantern/issuance/authorize"
                    );
                    assert_eq!(
                        request.headers().get(CONTENT_TYPE),
                        Some(&HeaderValue::from_static(
                            BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1
                        ))
                    );
                    assert_eq!(
                        request.headers().get(ACCEPT_ENCODING),
                        Some(&HeaderValue::from_static("identity"))
                    );
                    let authorization = request
                        .headers()
                        .get(AUTHORIZATION)
                        .expect("authorization header");
                    assert_eq!(
                        authorization,
                        HeaderValue::from_static("Bearer b3BhcXVlIGNyZWRlbnRpYWw")
                    );
                    assert!(authorization.is_sensitive());
                    assert!(!format!("{request:?}").contains("b3BhcXVlIGNyZWRlbnRpYWw"));
                    assert_eq!(
                        request.headers().get(ACCEPT),
                        Some(&HeaderValue::from_static(
                            BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1
                        ))
                    );
                    assert_eq!(
                        request.headers().get(CACHE_CONTROL),
                        Some(&HeaderValue::from_static("no-store"))
                    );
                    assert_eq!(
                        request.headers().get(PRAGMA),
                        Some(&HeaderValue::from_static("no-cache"))
                    );
                    assert_eq!(
                        request.body().and_then(reqwest::blocking::Body::as_bytes),
                        Some(&[][..])
                    );
                    Ok(response_v1(expected))
                },
            )
            .expect("exact response");
        assert_eq!(calls.get(), 1);
        assert_eq!(
            response.len(),
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1
        );
    }
    #[test]
    fn issue_length_is_rejected_before_network_access() {
        for invalid in [32, BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1 + 1] {
            let error = client_v1()
                .issue(&credential_v1(), &vec![0_u8; invalid])
                .expect_err("non-exact request must fail locally");
            assert_eq!(
                error,
                BootleLanternIssuanceClientErrorV1::InvalidIssueRequestLength
            );
        }
    }
    #[test]
    fn issue_magic_is_rejected_before_network_access() {
        for prefix in [*b"\0\0\0\0", *b"ILA0", *b"ILA\0", *b"XLA1"] {
            let mut request = patterned_v1(BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1);
            request[..4].copy_from_slice(&prefix);
            let error = client_v1()
                .issue(&credential_v1(), &request)
                .expect_err("same-length non-ILA1 request must fail locally");
            assert_eq!(
                error,
                BootleLanternIssuanceClientErrorV1::InvalidIssueRequestMagic
            );
        }
        for prefix in [*b"\0\0\0\0", *b"ILQ0", *b"ILQ\0", *b"XLQ1"] {
            let mut request = patterned_v1(BOOTLE_LANTERN_ISSUANCE_ISSUE_REQUEST_BYTES_V1);
            let offset = BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1;
            request[offset..offset + 4].copy_from_slice(&prefix);
            let error = client_v1()
                .issue(&credential_v1(), &request)
                .expect_err("same-length non-ILQ1 request must fail locally");
            assert_eq!(
                error,
                BootleLanternIssuanceClientErrorV1::InvalidIssueRequestMagic
            );
        }
    }
    #[test]
    fn transport_failure_is_not_retried() {
        let client = client_v1();
        let credential = credential_v1();
        let calls = Cell::new(0_u8);
        let error = client
            .execute_exact_with_v1(
                BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1,
                &credential,
                Vec::new(),
                BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1,
                |_request, _expected| {
                    calls.set(calls.get().saturating_add(1));
                    Err(BootleLanternIssuanceClientErrorV1::Network)
                },
            )
            .expect_err("transport failure must surface");
        assert_eq!(error, BootleLanternIssuanceClientErrorV1::Network);
        assert_eq!(calls.get(), 1);
    }
    #[test]
    fn response_metadata_and_lengths_fail_closed() {
        let expected = BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1;
        let mut status = response_v1(expected);
        status.status = StatusCode::TEMPORARY_REDIRECT;
        assert_eq!(
            validate_raw_response_v1(status, expected).expect_err("redirect must fail"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut content_type = response_v1(expected);
        content_type.headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-norito; charset=binary"),
        );
        assert_eq!(
            validate_raw_response_v1(content_type, expected).expect_err("media substitution"),
            BootleLanternIssuanceClientErrorV1::InvalidContentType
        );
        let mut missing_type = response_v1(expected);
        missing_type.headers.remove(CONTENT_TYPE);
        assert_eq!(
            validate_raw_response_v1(missing_type, expected).expect_err("missing media type"),
            BootleLanternIssuanceClientErrorV1::InvalidContentType
        );
        let mut duplicate_type = response_v1(expected);
        duplicate_type.headers.append(
            CONTENT_TYPE,
            HeaderValue::from_static(BOOTLE_LANTERN_ISSUANCE_CONTENT_TYPE_V1),
        );
        assert_eq!(
            validate_raw_response_v1(duplicate_type, expected).expect_err("duplicate media type"),
            BootleLanternIssuanceClientErrorV1::InvalidContentType
        );
        let mut encoded = response_v1(expected);
        encoded
            .headers
            .insert(CONTENT_ENCODING, HeaderValue::from_static("identity"));
        assert_eq!(
            validate_raw_response_v1(encoded, expected).expect_err("encoding header must fail"),
            BootleLanternIssuanceClientErrorV1::ContentEncoding
        );
        let mut challenged = response_v1(expected);
        challenged.headers.insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"iroha-bootle-lantern-issuance\""),
        );
        assert_eq!(
            validate_raw_response_v1(challenged, expected)
                .expect_err("successful response challenge must fail"),
            BootleLanternIssuanceClientErrorV1::UnexpectedAuthenticationChallenge
        );
        for invalid_length in ["0320", "319", "321", "+320", "320, 320"] {
            let mut response = response_v1(expected);
            response.headers.insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(invalid_length).expect("valid header syntax"),
            );
            assert_eq!(
                validate_raw_response_v1(response, expected)
                    .expect_err("invalid declared length must fail"),
                BootleLanternIssuanceClientErrorV1::InvalidContentLength,
                "length {invalid_length:?}"
            );
        }
        let mut duplicate_length = response_v1(expected);
        duplicate_length.headers.append(
            CONTENT_LENGTH,
            HeaderValue::from_str(&expected.to_string()).expect("valid length header"),
        );
        assert_eq!(
            validate_raw_response_v1(duplicate_length, expected)
                .expect_err("duplicate declared length must fail"),
            BootleLanternIssuanceClientErrorV1::InvalidContentLength
        );
        let mut omitted_length = response_v1(expected);
        omitted_length.headers.remove(CONTENT_LENGTH);
        assert_eq!(
            validate_raw_response_v1(omitted_length, expected)
                .expect("an exact bounded body does not require Content-Length")
                .len(),
            expected
        );
        for actual in [expected - 1, expected + 1] {
            let mut response = response_v1(expected);
            response.body.resize(actual, 0);
            assert_eq!(
                validate_raw_response_v1(response, expected)
                    .expect_err("invalid body length must fail"),
                BootleLanternIssuanceClientErrorV1::InvalidResponseLength
            );
        }
        for prefix in [*b"\0\0\0\0", *b"ILA0", *b"ILA\0", *b"XLA1"] {
            let mut response = response_v1(expected);
            response.body[..4].copy_from_slice(&prefix);
            assert_eq!(
                validate_raw_response_v1(response, expected)
                    .expect_err("same-length non-ILA1 response must fail"),
                BootleLanternIssuanceClientErrorV1::InvalidResponseMagic
            );
        }
        for prefix in [*b"\0\0\0\0", *b"ILR0", *b"ILR\0", *b"XLR1"] {
            let expected = BOOTLE_LANTERN_ISSUANCE_ISSUE_RESPONSE_BYTES_V1;
            let mut response = response_v1(expected);
            response.body[..4].copy_from_slice(&prefix);
            assert_eq!(
                validate_raw_response_v1(response, expected)
                    .expect_err("same-length non-ILR1 response must fail"),
                BootleLanternIssuanceClientErrorV1::InvalidResponseMagic
            );
        }
    }
    #[test]
    fn structured_error_responses_fail_closed() {
        let expected = BOOTLE_LANTERN_ISSUANCE_AUTHORIZATION_RESPONSE_BYTES_V1;
        for (status, code) in [
            (StatusCode::BAD_REQUEST, "privacy_issuance_invalid_request"),
            (StatusCode::UNAUTHORIZED, "privacy_issuance_unauthorized"),
            (
                StatusCode::NOT_ACCEPTABLE,
                "privacy_issuance_not_acceptable",
            ),
            (StatusCode::CONFLICT, "privacy_issuance_state_conflict"),
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                "privacy_issuance_payload_too_large",
            ),
            (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "privacy_issuance_unsupported_media_type",
            ),
            (
                StatusCode::TOO_MANY_REQUESTS,
                "privacy_issuance_capacity_exhausted",
            ),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "privacy_issuance_unavailable",
            ),
        ] {
            assert_eq!(
                validate_raw_response_v1(error_response_v1(status, code), expected)
                    .expect_err("canonical error must surface"),
                BootleLanternIssuanceClientErrorV1::Http {
                    status: status.as_u16(),
                    code: code.to_owned(),
                    retry_after_seconds: (status == StatusCode::TOO_MANY_REQUESTS).then_some(1),
                }
            );
        }
        let mut missing_retry = error_response_v1(
            StatusCode::TOO_MANY_REQUESTS,
            "privacy_issuance_capacity_exhausted",
        );
        missing_retry.headers.remove(RETRY_AFTER);
        assert_eq!(
            validate_raw_response_v1(missing_retry, expected).expect_err("missing retry hint"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut unexpected_retry = error_response_v1(
            StatusCode::SERVICE_UNAVAILABLE,
            "privacy_issuance_unavailable",
        );
        unexpected_retry
            .headers
            .insert(RETRY_AFTER, HeaderValue::from_static("1"));
        assert_eq!(
            validate_raw_response_v1(unexpected_retry, expected)
                .expect_err("unexpected retry hint"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut missing_challenge =
            error_response_v1(StatusCode::UNAUTHORIZED, "privacy_issuance_unauthorized");
        missing_challenge.headers.remove(WWW_AUTHENTICATE);
        assert_eq!(
            validate_raw_response_v1(missing_challenge, expected)
                .expect_err("missing authentication challenge"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut duplicate_challenge =
            error_response_v1(StatusCode::UNAUTHORIZED, "privacy_issuance_unauthorized");
        duplicate_challenge.headers.append(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"iroha-bootle-lantern-issuance\""),
        );
        assert_eq!(
            validate_raw_response_v1(duplicate_challenge, expected)
                .expect_err("duplicate authentication challenge"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut substituted_challenge =
            error_response_v1(StatusCode::UNAUTHORIZED, "privacy_issuance_unauthorized");
        substituted_challenge.headers.insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"attacker\""),
        );
        assert_eq!(
            validate_raw_response_v1(substituted_challenge, expected)
                .expect_err("substituted authentication challenge"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut unexpected_challenge =
            error_response_v1(StatusCode::BAD_REQUEST, "privacy_issuance_invalid_request");
        unexpected_challenge.headers.insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"iroha-bootle-lantern-issuance\""),
        );
        assert_eq!(
            validate_raw_response_v1(unexpected_challenge, expected)
                .expect_err("authentication challenge is forbidden outside 401"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut wrong_media =
            error_response_v1(StatusCode::BAD_REQUEST, "privacy_issuance_invalid_request");
        wrong_media
            .headers
            .insert(CONTENT_TYPE, HeaderValue::from_static(JSON_MEDIA_TYPE_V1));
        assert_eq!(
            validate_raw_response_v1(wrong_media, expected).expect_err("wrong media type"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut corrupted =
            error_response_v1(StatusCode::BAD_REQUEST, "privacy_issuance_invalid_request");
        corrupted.body[0] ^= 0x01;
        corrupted.headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&corrupted.body.len().to_string()).expect("valid length"),
        );
        assert_eq!(
            validate_raw_response_v1(corrupted, expected).expect_err("corrupt Norito frame"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut wrong_code =
            error_response_v1(StatusCode::BAD_REQUEST, "privacy_issuance_unauthorized");
        wrong_code.headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&wrong_code.body.len().to_string()).expect("valid length"),
        );
        assert_eq!(
            validate_raw_response_v1(wrong_code, expected).expect_err("status/code mismatch"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
        let mut noncanonical_json = error_response_v1(
            StatusCode::NOT_ACCEPTABLE,
            "privacy_issuance_not_acceptable",
        );
        noncanonical_json.body.push(b' ');
        noncanonical_json.headers.insert(
            CONTENT_LENGTH,
            HeaderValue::from_str(&noncanonical_json.body.len().to_string()).expect("valid length"),
        );
        assert_eq!(
            validate_raw_response_v1(noncanonical_json, expected).expect_err("noncanonical JSON"),
            BootleLanternIssuanceClientErrorV1::InvalidErrorResponse
        );
    }
}
