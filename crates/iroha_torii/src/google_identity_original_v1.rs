//! Original Google ID-token verification for first-device hardware-evidence DATA.
//!
//! This producer owns its fixed Google HTTPS/JWKS transport and verifies the exact JWS.
//! Its output does not admit the supplied public manifest or create an issuer, reservation,
//! Native installation, account, Current or monetary authority. The future issuer must own
//! those separate authenticated inputs and recheck current validity before signing/persisting.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt as _;
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaOrdinaryNativeClockOwnerV1, KagemushaOrdinaryNativeTimeIntervalV1,
};
use iroha_data_model::kagemusha::{
    KagemushaHardwareEvidenceBootstrapManifestV1,
    kagemusha_hardware_evidence_google_owner_binding_v1,
};
use jsonwebtoken::{Algorithm, DecodingKey};
use norito::json::{self, Value};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex as NativeClockMutex},
    time::{Duration, Instant},
};

// Reuse the maintained FI Google provider and its finite transport/cache limits.
const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const MAX_ORIGINAL_BYTES: usize = 16 * 1024;
const MAX_JWKS_BYTES: usize = 256 * 1024;
const MAX_JWKS_KEYS: usize = 64;
const MAX_KID_BYTES: usize = 128;
const FRESH_SECONDS: u64 = 300;
const STALE_SECONDS: u64 = 3_600;
const UNKNOWN_KID_REFRESH_SECONDS: u64 = 30;
const MAX_INFLIGHT: usize = 32;

/// Closed lower-verification failures; token, claims, keys and private clock material stay out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KagemushaGoogleIdOriginalErrorV1 {
    /// Original shape, signature, selected claim or complete validity interval differs.
    #[error("first-device Google original rejected")]
    Rejected,
    /// Bounded provider transport/cache or concurrent verification capacity is unavailable.
    #[error("first-device Google provider unavailable")]
    ProviderUnavailable,
    /// The actual held Native clock or its durable original custody is unavailable.
    #[error("first-device Google clock custody unavailable")]
    ClockCustody,
}
type Result<T> = core::result::Result<T, KagemushaGoogleIdOriginalErrorV1>;
use KagemushaGoogleIdOriginalErrorV1::{ClockCustody, ProviderUnavailable, Rejected};

/// Lower original-verification DATA, not a manifest/issuer/account/Current authority.
///
/// Private fields prohibit accidental decoded-result injection into this API. They do not
/// establish an authority boundary, and this value supplies no signer or accepting callback.
pub struct KagemushaVerifiedGoogleIdOriginalDataV1 {
    token_original_sha256: [u8; 32],
    manifest_digest: [u8; 32],
    google_owner_binding_digest: [u8; 32],
    issuer: String,
    audience: String,
    subject: String,
    issued_at_seconds: u64,
    expires_at_seconds: u64,
    observed_native_time_interval: KagemushaOrdinaryNativeTimeIntervalV1,
}
impl KagemushaVerifiedGoogleIdOriginalDataV1 {
    /// SHA256 of the complete untouched ASCII original, not decoded/re-encoded claims.
    #[must_use]
    pub fn token_original_sha256(&self) -> [u8; 32] {
        self.token_original_sha256
    }
    /// Existing Model purpose-bound canonical manifest identity; no release admission implied.
    #[must_use]
    pub fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }
    /// Existing exact manifest/issuer/audience/subject naming digest; no owner grant implied.
    #[must_use]
    pub fn google_owner_binding_digest(&self) -> [u8; 32] {
        self.google_owner_binding_digest
    }
    /// Exact signed issuer, equal to this verification's manifest-selected literal.
    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    /// Exact signed scalar audience, equal to this verification's selected client ID.
    #[must_use]
    pub fn audience(&self) -> &str {
        &self.audience
    }
    /// Exact bounded signed subject; neither an account mapping nor verified-email login.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }
    /// Actual checked positive signed `iat`, in seconds.
    #[must_use]
    pub fn issued_at_seconds(&self) -> u64 {
        self.issued_at_seconds
    }
    /// Actual checked positive signed exclusive `exp`, in seconds.
    #[must_use]
    pub fn expires_at_seconds(&self) -> u64 {
        self.expires_at_seconds
    }
    /// Actual post-await/post-signature interval observed under the same held clock selection.
    /// A consumer must reobserve validity at its later signing/persistence boundary.
    #[must_use]
    pub fn observed_native_time_interval(&self) -> KagemushaOrdinaryNativeTimeIntervalV1 {
        self.observed_native_time_interval
    }
}

#[derive(Clone)]
struct GoogleJwk {
    kid: String,
    kty: Option<String>,
    alg: Option<String>,
    use_field: Option<String>,
    n: Option<String>,
    e: Option<String>,
}
struct CachedGoogleJwks {
    keys: Arc<Vec<GoogleJwk>>,
    fetched_at: Instant,
}

/// Owns the sole fixed Google provider transport and a private bounded rotating JWKS cache.
///
/// There is no constructor accepting a client, URL, offered JWKS, key, claims, scalar time or
/// supplied verification result. This lower producer does not own/admit the offered manifest.
pub struct KagemushaGoogleIdOriginalVerifierV1 {
    http: reqwest::Client,
    cache: tokio::sync::Mutex<Option<CachedGoogleJwks>>,
    inflight: tokio::sync::Semaphore,
}
impl KagemushaGoogleIdOriginalVerifierV1 {
    /// Build the owned fixed Google client/cache without making a provider request.
    ///
    /// # Errors
    /// Refuses unavailable client construction. No alternate transport or fallback is accepted.
    pub fn new() -> core::result::Result<Self, KagemushaGoogleIdOriginalErrorV1> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(4))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ProviderUnavailable)?;
        Ok(Self {
            http,
            cache: tokio::sync::Mutex::new(None),
            inflight: tokio::sync::Semaphore::new(MAX_INFLIGHT),
        })
    }

    /// Verify original RS256 Google bytes under exact public manifest selections and the
    /// actual held Native clock. All returned bindings remain lower verification DATA.
    ///
    /// The outer issuer must admit that same manifest and own its signer, reservation/replay,
    /// nonce/alias/challenge and durable journal. This method constructs none of those owners.
    ///
    /// # Errors
    /// Refuses malformed originals/policy, unavailable provider or held clock, false signatures,
    /// different literal claims or a current interval outside the signed validity windows.
    pub async fn verify_first_device_original(
        &self,
        token_original: &str,
        manifest: &KagemushaHardwareEvidenceBootstrapManifestV1,
        clock: &NativeClockMutex<KagemushaOrdinaryNativeClockOwnerV1>,
    ) -> core::result::Result<
        KagemushaVerifiedGoogleIdOriginalDataV1,
        KagemushaGoogleIdOriginalErrorV1,
    > {
        compact_original(token_original)?;
        // Validate bounded public fields before copying them; this is not release admission.
        manifest.validate().map_err(|_| Rejected)?;
        let selected = manifest.clone();
        let manifest_digest = selected.digest().map_err(|_| Rejected)?;
        let _permit = self
            .inflight
            .try_acquire()
            .map_err(|_| ProviderUnavailable)?;
        let token = token_original.to_owned();
        let parts = compact_original(&token)?;
        let kid = parse_original_header(parts.header, &token)?;
        // The synchronous guard ends inside this helper, before the provider/cache await.
        current_manifest_interval(clock, &selected)?;
        let key = self.cached_google_key(&kid).await?;
        verify_original_signature(&parts, &key)?;
        // Claims become trusted signed DATA only after the actual RS256 boolean is true.
        let payload = URL_SAFE_NO_PAD
            .decode(parts.payload)
            .map_err(|_| Rejected)?;
        let claims = parse_signed_claims(
            &payload,
            &selected.google_oauth_issuer,
            &selected.google_oauth_client_id,
        )?;
        // A network wait cannot reuse an early reading or JWT's default UTC/leeway.
        let interval = current_manifest_interval(clock, &selected)?;
        interval
            .require_validity(claims.not_before_ms, claims.expires_at_ms)
            .map_err(|_| Rejected)?;
        let google_owner_binding_digest = kagemusha_hardware_evidence_google_owner_binding_v1(
            manifest_digest,
            &claims.issuer,
            &claims.audience,
            &claims.subject,
        )
        .map_err(|_| Rejected)?;
        Ok(KagemushaVerifiedGoogleIdOriginalDataV1 {
            token_original_sha256: Sha256::digest(token.as_bytes()).into(),
            manifest_digest,
            google_owner_binding_digest,
            issuer: claims.issuer,
            audience: claims.audience,
            subject: claims.subject,
            issued_at_seconds: claims.issued_at_seconds,
            expires_at_seconds: claims.expires_at_seconds,
            observed_native_time_interval: interval,
        })
    }

    async fn cached_google_key(&self, kid: &str) -> Result<GoogleJwk> {
        validate_kid(kid)?;
        // Single-flight refresh: no std clock guard is held across this await.
        let mut guard = self.cache.lock().await;
        let now = Instant::now();
        if let Some(cached) = guard.as_ref() {
            let age = now.saturating_duration_since(cached.fetched_at);
            if let Some(key) = key_with_kid(&cached.keys, kid) {
                if cache_fresh(age) {
                    return Ok(key);
                }
            } else if suppress_unknown_refresh(age) {
                return Err(Rejected);
            }
        }
        match self.fetch_google_jwks().await {
            Ok(keys) => {
                let selected = key_with_kid(&keys, kid);
                // Complete replacement; rotated-out keys are not kept in an additive union.
                *guard = Some(CachedGoogleJwks {
                    keys: Arc::new(keys),
                    fetched_at: Instant::now(),
                });
                selected.ok_or(Rejected)
            }
            Err(error) => {
                if let Some(cached) = guard.as_ref() {
                    // Measure after the failed await; request time cannot extend stale bounds.
                    let age = Instant::now().saturating_duration_since(cached.fetched_at);
                    if cache_stale_allowed(age) {
                        if let Some(key) = key_with_kid(&cached.keys, kid) {
                            return Ok(key);
                        }
                    }
                }
                Err(error)
            }
        }
    }

    async fn fetch_google_jwks(&self) -> Result<Vec<GoogleJwk>> {
        let response = self
            .http
            .get(GOOGLE_JWKS_URL)
            .send()
            .await
            .map_err(|_| ProviderUnavailable)?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|length| length > MAX_JWKS_BYTES as u64)
        {
            return Err(ProviderUnavailable);
        }
        let mut payload = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| ProviderUnavailable)?;
            append_jwks_chunk(&mut payload, &chunk)?;
        }
        parse_google_jwks(&payload)
    }
}

fn current_manifest_interval(
    clock: &NativeClockMutex<KagemushaOrdinaryNativeClockOwnerV1>,
    manifest: &KagemushaHardwareEvidenceBootstrapManifestV1,
) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
    let mut owner = clock.lock().map_err(|_| ClockCustody)?;
    if *owner.network_id().map_err(|_| ClockCustody)?.as_bytes() != manifest.network_id
        || owner
            .installed_selection_digest()
            .map_err(|_| ClockCustody)?
            != manifest.native_clock_selection_digest
    {
        return Err(Rejected);
    }
    let interval = owner
        .current_native_time_interval()
        .map_err(|_| ClockCustody)?;
    interval
        .require_validity(manifest.not_before_ms, manifest.expires_at_ms)
        .map_err(|_| Rejected)?;
    Ok(interval)
}

struct CompactOriginal<'a> {
    header: &'a str,
    payload: &'a str,
    signature: &'a str,
    signing_input: &'a str,
}
fn compact_original(token: &str) -> Result<CompactOriginal<'_>> {
    if token.is_empty()
        || token.len() > MAX_ORIGINAL_BYTES
        || !token.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(Rejected);
    }
    let (signing_input, signature) = token.rsplit_once('.').ok_or(Rejected)?;
    let (header, payload) = signing_input.split_once('.').ok_or(Rejected)?;
    if header.is_empty() || payload.is_empty() || signature.is_empty() || payload.contains('.') {
        return Err(Rejected);
    }
    Ok(CompactOriginal {
        header,
        payload,
        signature,
        signing_input,
    })
}

fn parse_original_header(encoded: &str, original: &str) -> Result<String> {
    let decoded = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| Rejected)?;
    let value: Value = json::from_slice(&decoded).map_err(|_| Rejected)?;
    let object = value.as_object().ok_or(Rejected)?;
    if object.get("alg").and_then(Value::as_str) != Some("RS256")
        || object.contains_key("crit")
        || object
            .get("b64")
            .is_some_and(|value| !matches!(value, Value::Bool(true)))
    {
        return Err(Rejected);
    }
    let kid = object.get("kid").and_then(Value::as_str).ok_or(Rejected)?;
    validate_kid(kid)?;
    let header = jsonwebtoken::decode_header(original).map_err(|_| Rejected)?;
    if header.alg != Algorithm::RS256 || header.kid.as_deref() != Some(kid) {
        return Err(Rejected);
    }
    // Untrusted jku/jwk/x5u/x5c are never consulted for the fixed provider/key selection.
    Ok(kid.to_owned())
}

fn validate_kid(kid: &str) -> Result<()> {
    if kid.is_empty() || kid.len() > MAX_KID_BYTES || kid.trim() != kid {
        Err(Rejected)
    } else {
        Ok(())
    }
}
fn optional_string(value: Option<&Value>) -> Result<Option<String>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(Rejected),
    }
}
fn parse_google_jwks(payload: &[u8]) -> Result<Vec<GoogleJwk>> {
    if payload.len() > MAX_JWKS_BYTES {
        return Err(ProviderUnavailable);
    }
    let value: Value = json::from_slice(payload).map_err(|_| Rejected)?;
    let keys = value
        .as_object()
        .and_then(|object| object.get("keys"))
        .and_then(Value::as_array)
        .ok_or(Rejected)?;
    if keys.is_empty() || keys.len() > MAX_JWKS_KEYS {
        return Err(Rejected);
    }
    let mut kids = BTreeSet::new();
    let mut parsed = Vec::with_capacity(keys.len());
    for key in keys {
        let object = key.as_object().ok_or(Rejected)?;
        let kid = object.get("kid").and_then(Value::as_str).ok_or(Rejected)?;
        validate_kid(kid)?;
        if !kids.insert(kid.to_owned()) {
            return Err(Rejected);
        }
        parsed.push(GoogleJwk {
            kid: kid.to_owned(),
            kty: optional_string(object.get("kty"))?,
            alg: optional_string(object.get("alg"))?,
            use_field: optional_string(object.get("use"))?,
            n: optional_string(object.get("n"))?,
            e: optional_string(object.get("e"))?,
        });
    }
    Ok(parsed)
}
fn key_with_kid(keys: &[GoogleJwk], kid: &str) -> Option<GoogleJwk> {
    keys.iter().find(|key| key.kid == kid).cloned()
}
fn append_jwks_chunk(payload: &mut Vec<u8>, chunk: &[u8]) -> Result<()> {
    if payload
        .len()
        .checked_add(chunk.len())
        .is_none_or(|size| size > MAX_JWKS_BYTES)
    {
        return Err(ProviderUnavailable);
    }
    payload.extend_from_slice(chunk);
    Ok(())
}
fn cache_fresh(age: Duration) -> bool {
    age <= Duration::from_secs(FRESH_SECONDS)
}
fn suppress_unknown_refresh(age: Duration) -> bool {
    age <= Duration::from_secs(UNKNOWN_KID_REFRESH_SECONDS)
}
fn cache_stale_allowed(age: Duration) -> bool {
    age <= Duration::from_secs(STALE_SECONDS)
}
fn verify_original_signature(parts: &CompactOriginal<'_>, key: &GoogleJwk) -> Result<()> {
    if key.kty.as_deref() != Some("RSA")
        || key.alg.as_deref() != Some("RS256")
        || key.use_field.as_deref() != Some("sig")
    {
        return Err(Rejected);
    }
    let decoding_key = DecodingKey::from_rsa_components(
        key.n.as_deref().ok_or(Rejected)?,
        key.e.as_deref().ok_or(Rejected)?,
    )
    .map_err(|_| Rejected)?;
    match jsonwebtoken::crypto::verify(
        parts.signature,
        parts.signing_input.as_bytes(),
        &decoding_key,
        Algorithm::RS256,
    ) {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => Err(Rejected),
    }
}

// Pure parsed DATA only. The public method calls this after actual original signature true.
struct ParsedGoogleClaims {
    issuer: String,
    audience: String,
    subject: String,
    issued_at_seconds: u64,
    expires_at_seconds: u64,
    not_before_ms: u64,
    expires_at_ms: u64,
}
fn identity_string(value: Option<&Value>) -> Result<String> {
    let value = value.and_then(Value::as_str).ok_or(Rejected)?;
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(Rejected);
    }
    Ok(value.to_owned())
}
fn positive_seconds(value: Option<&Value>) -> Result<(u64, u64)> {
    let seconds = value
        .and_then(Value::as_u64)
        .filter(|value| *value != 0)
        .ok_or(Rejected)?;
    Ok((seconds, seconds.checked_mul(1_000).ok_or(Rejected)?))
}
fn parse_signed_claims(payload: &[u8], issuer: &str, audience: &str) -> Result<ParsedGoogleClaims> {
    let value: Value = json::from_slice(payload).map_err(|_| Rejected)?;
    let object = value.as_object().ok_or(Rejected)?;
    let claimed_issuer = identity_string(object.get("iss"))?;
    let claimed_audience = identity_string(object.get("aud"))?;
    let subject = identity_string(object.get("sub"))?;
    if claimed_issuer != issuer || claimed_audience != audience {
        return Err(Rejected);
    }
    let (issued_at_seconds, issued_at_ms) = positive_seconds(object.get("iat"))?;
    let (expires_at_seconds, expires_at_ms) = positive_seconds(object.get("exp"))?;
    if issued_at_ms >= expires_at_ms {
        return Err(Rejected);
    }
    let not_before_ms = match object.get("nbf") {
        None => issued_at_ms,
        Some(value) => issued_at_ms.max(
            value
                .as_u64()
                .ok_or(Rejected)?
                .checked_mul(1_000)
                .ok_or(Rejected)?,
        ),
    };
    if not_before_ms >= expires_at_ms {
        return Err(Rejected);
    }
    Ok(ParsedGoogleClaims {
        issuer: claimed_issuer,
        audience: claimed_audience,
        subject,
        issued_at_seconds,
        expires_at_seconds,
        not_before_ms,
        expires_at_ms,
    })
}

#[cfg(test)]
mod tests {
    // UNEXECUTED SOURCE-COPY controls. These pure parser/cache/invalid-signature inputs
    // supply neither Google provenance nor a genuine Native/manifest/issuer authority.
    use super::*;

    fn data_token(header: &str, payload: &str, signature: &str) -> String {
        format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode(header),
            URL_SAFE_NO_PAD.encode(payload),
            signature
        )
    }
    fn data_claims(iat: &str, exp: &str, extra: &str) -> String {
        format!(
            r#"{{"iss":"https://accounts.google.com","aud":"selected-client","sub":"data-subject","iat":{iat},"exp":{exp}{extra}}}"#
        )
    }
    fn parsed_data(payload: &str) -> Result<ParsedGoogleClaims> {
        parse_signed_claims(
            payload.as_bytes(),
            "https://accounts.google.com",
            "selected-client",
        )
    }

    #[test]
    fn compact_original_bounds_and_signing_bytes_are_exact() {
        let parts = compact_original("a.b.c").unwrap();
        assert_eq!(parts.signing_input, "a.b");
        assert_eq!(parts.signature, "c");
        for value in [
            "", " a.b.c", "a.b.c ", "a.\nb.c", "a.é.c", "a.b", ".b.c", "a..c", "a.b.", "a.b.c.d",
        ] {
            assert!(compact_original(value).is_err());
        }
        assert!(compact_original(&format!("a.b.{}", "c".repeat(MAX_ORIGINAL_BYTES - 4))).is_ok());
        assert!(compact_original(&format!("a.b.{}", "c".repeat(MAX_ORIGINAL_BYTES - 3))).is_err());
    }

    #[test]
    fn header_uses_actual_original_base64_json_and_rs256_selector() {
        let token = data_token(
            r#"{"alg":"RS256","kid":"data-key","jku":"https://offered.invalid/jwks"}"#,
            "{}",
            "c",
        );
        let parts = compact_original(&token).unwrap();
        assert_eq!(
            parse_original_header(parts.header, &token).unwrap(),
            "data-key"
        );
        // Accepting an untrusted routing label does not fetch this offered URL or verify a token.
        for header in [
            r#"{"alg":"HS256","kid":"data-key"}"#,
            r#"{"alg":"RS256"}"#,
            r#"{"alg":"RS256","kid":null}"#,
            r#"{"alg":"RS256","kid":"data-key","kid":"other"}"#,
            r#"{"alg":"RS256","kid":"data-key","crit":[]}"#,
            r#"{"alg":"RS256","kid":"data-key","b64":false}"#,
            r#"{"alg":"RS256","kid":"data-key"}{}"#,
        ] {
            let token = data_token(header, "{}", "c");
            assert!(
                parse_original_header(compact_original(&token).unwrap().header, &token).is_err()
            );
        }
        assert!(parse_original_header("!", "!.e30.c").is_err());
        let bad_utf8 = format!("{}.e30.c", URL_SAFE_NO_PAD.encode([0xff]));
        assert!(
            parse_original_header(compact_original(&bad_utf8).unwrap().header, &bad_utf8).is_err()
        );
    }

    #[test]
    fn kid_limits_and_jwks_cardinality_duplicates_are_closed() {
        assert!(validate_kid(&"k".repeat(MAX_KID_BYTES)).is_ok());
        for kid in [
            "".to_owned(),
            " key".to_owned(),
            "key ".to_owned(),
            "k".repeat(MAX_KID_BYTES + 1),
        ] {
            assert!(validate_kid(&kid).is_err());
        }
        assert!(parse_google_jwks(br#"{"keys":[{"kid":"data-key"}]}"#).is_ok());
        for bytes in [
            br#"{"keys":[]}"#.as_slice(),
            br#"{"keys":[{"kid":"k"},{"kid":"k"}]}"#.as_slice(),
            br#"{"keys":[{"kid":"k","kid":"other"}]}"#.as_slice(),
            br#"{"keys":[{"kid":"k","n":3}]}"#.as_slice(),
        ] {
            assert!(parse_google_jwks(bytes).is_err());
        }
        let keys = (0..MAX_JWKS_KEYS)
            .map(|n| format!(r#"{{"kid":"k{n}"}}"#))
            .collect::<Vec<_>>();
        assert!(
            parse_google_jwks(format!(r#"{{"keys":[{}]}}"#, keys.join(",")).as_bytes()).is_ok()
        );
        let mut extra = keys;
        extra.push(r#"{"kid":"extra"}"#.to_owned());
        assert!(
            parse_google_jwks(format!(r#"{{"keys":[{}]}}"#, extra.join(",")).as_bytes()).is_err()
        );
    }

    #[test]
    fn complete_jwks_chunk_bound_refuses_plus_one_without_appending() {
        let mut bytes = vec![0; MAX_JWKS_BYTES - 1];
        append_jwks_chunk(&mut bytes, &[0]).unwrap();
        assert_eq!(bytes.len(), MAX_JWKS_BYTES);
        assert_eq!(
            append_jwks_chunk(&mut bytes, &[0]),
            Err(ProviderUnavailable)
        );
        assert_eq!(bytes.len(), MAX_JWKS_BYTES);
        assert!(parse_google_jwks(&vec![0; MAX_JWKS_BYTES + 1]).is_err());
    }

    #[test]
    fn monotonic_cache_bounds_and_rotation_have_no_forever_union() {
        let bounds: [(u64, fn(Duration) -> bool); 3] = [
            (FRESH_SECONDS, cache_fresh),
            (STALE_SECONDS, cache_stale_allowed),
            (UNKNOWN_KID_REFRESH_SECONDS, suppress_unknown_refresh),
        ];
        for (seconds, predicate) in bounds {
            assert!(predicate(Duration::from_secs(seconds)));
            assert!(!predicate(
                Duration::from_secs(seconds) + Duration::from_nanos(1)
            ));
        }
        let old = parse_google_jwks(br#"{"keys":[{"kid":"old"}]}"#).unwrap();
        let replacement = parse_google_jwks(br#"{"keys":[{"kid":"new"}]}"#).unwrap();
        assert!(key_with_kid(&old, "old").is_some());
        assert!(key_with_kid(&replacement, "old").is_none());
        assert!(key_with_kid(&replacement, "new").is_some());
        assert!(key_with_kid(&old, "unknown").is_none());
    }

    #[test]
    fn actual_invalid_rsa_signature_result_cannot_pass_as_ok_bool() {
        // Deliberately invalid public components, not an accepted provider key/fixture.
        let key = parse_google_jwks(br#"{"keys":[{"kid":"data","kty":"RSA","alg":"RS256","use":"sig","n":"AQ","e":"AQAB"}]}"#).unwrap().remove(0);
        let original = compact_original("e30.e30.AQ").unwrap();
        let decoding_key = DecodingKey::from_rsa_components("AQ", "AQAB").unwrap();
        assert!(matches!(
            jsonwebtoken::crypto::verify(
                original.signature,
                original.signing_input.as_bytes(),
                &decoding_key,
                Algorithm::RS256
            ),
            Ok(false)
        ));
        assert_eq!(verify_original_signature(&original, &key), Err(Rejected));
        assert_eq!(
            verify_original_signature(&compact_original("e30.e30.!").unwrap(), &key),
            Err(Rejected)
        );
        assert_eq!(
            verify_original_signature(&compact_original("e30.changed.AQ").unwrap(), &key),
            Err(Rejected)
        );
    }

    #[test]
    fn selected_key_metadata_and_missing_components_refuse() {
        for key in [
            r#"{"kid":"data","kty":"EC","alg":"RS256","use":"sig","n":"AQ","e":"AQAB"}"#,
            r#"{"kid":"data","kty":"RSA","alg":"HS256","use":"sig","n":"AQ","e":"AQAB"}"#,
            r#"{"kid":"data","kty":"RSA","alg":"RS256","use":"enc","n":"AQ","e":"AQAB"}"#,
            r#"{"kid":"data","kty":"RSA","alg":"RS256","use":"sig","e":"AQAB"}"#,
            r#"{"kid":"data","kty":"RSA","alg":"RS256","use":"sig","n":"AQ"}"#,
        ] {
            let key = parse_google_jwks(format!(r#"{{"keys":[{key}]}}"#).as_bytes())
                .unwrap()
                .remove(0);
            assert_eq!(
                verify_original_signature(&compact_original("e30.e30.AQ").unwrap(), &key),
                Err(Rejected)
            );
        }
    }

    #[test]
    fn parsed_claim_data_preserves_exact_literals_and_never_admits_an_owner() {
        let parsed = parsed_data(&data_claims("1", "3", "")).unwrap();
        assert_eq!(parsed.issuer, "https://accounts.google.com");
        assert_eq!(parsed.audience, "selected-client");
        assert_eq!((parsed.not_before_ms, parsed.expires_at_ms), (1_000, 3_000));
        let good = data_claims("1", "3", "");
        for wrong in [
            good.replace("https://accounts.google.com", "accounts.google.com"),
            good.replace("selected-client", "other-client"),
            good.replace("\"selected-client\"", "[\"selected-client\"]"),
            good.replace("\"data-subject\"", "null"),
            good.replace("data-subject", ""),
            good.replace("data-subject", "bad\\nsubject"),
            good.replace("data-subject", &"s".repeat(513)),
            good.replace("\"iss\":", "\"iss\":\"other\",\"iss\":"),
            format!("{good}{{}}"),
        ] {
            assert!(parsed_data(&wrong).is_err());
        }
    }

    #[test]
    fn actual_signed_timestamp_shapes_order_and_checked_milliseconds_refuse() {
        for number in [
            "0",
            "-1",
            "1.0",
            "\"1\"",
            "null",
            "18446744073709551616",
            "18446744073709552",
        ] {
            assert!(parsed_data(&data_claims(number, "3", "")).is_err());
            assert!(parsed_data(&data_claims("1", number, "")).is_err());
        }
        assert!(parsed_data(&data_claims("3", "3", "")).is_err());
        assert!(parsed_data(&data_claims("4", "3", "")).is_err());
        let maximum = (u64::MAX / 1_000).to_string();
        assert_eq!(
            parsed_data(&data_claims("1", &maximum, ""))
                .unwrap()
                .expires_at_ms,
            (u64::MAX / 1_000) * 1_000
        );
    }

    #[test]
    fn optional_actual_nbf_zero_and_checked_max_do_not_supply_current_time() {
        assert_eq!(
            parsed_data(&data_claims("1", "3", ",\"nbf\":0"))
                .unwrap()
                .not_before_ms,
            1_000
        );
        assert_eq!(
            parsed_data(&data_claims("1", "3", ",\"nbf\":2"))
                .unwrap()
                .not_before_ms,
            2_000
        );
        for nbf in ["-1", "1.0", "null", "\"1\"", "3", "18446744073709552"] {
            assert!(parsed_data(&data_claims("1", "3", &format!(",\"nbf\":{nbf}"))).is_err());
        }
        // No interval/Native clock/verified-result fixture is created by these parse controls.
    }
}
