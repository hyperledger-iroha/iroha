//! Closed Android FI HTTP proof DATA under a genuinely held ordinary identity.
//!
//! FI token/JWT admission remains at FI. This owner neither creates a session nor grants
//! ledger, retail or monetary authority. Native must retain the exact final request and
//! source-selected FI pins, and recheck actual account/key retirement around preparation,
//! every loan projection, platform signing, completion and dispatch. This transient holder
//! is not a durable invocation journal: dropping it after a possible call permits no retry.
//! TODO: wire genuine Native account retirement, platform-original custody and final transport
//! snapshot consumption before exposing this family through the managed coordinator.

use super::{
    Custody, KagemushaOrdinaryAppPossessionAttemptV1, KagemushaOrdinaryIdentityErrorV1,
    KagemushaOrdinaryPreparationSelectedOriginalsV1, KagemushaPendingAppIdentityV1, Rejected,
    Result,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_data_model::account::AccountId;
use iroha_data_model::kagemusha::{
    KagemushaAppKeySecurityLevelV1, KagemushaHardwarePlatformClassV1,
    kagemusha_ordinary_app_account_binding_v1,
};
use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier as _};
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::sync::Arc;
use url::Url;
use zeroize::Zeroizing;

const MAX_TOKEN_BYTES: usize = 16_384;
const MAX_HTU_BYTES: usize = 2_048;
const MAX_OPERATION_BYTES: usize = 256;
const MAX_PROOF_BYTES: usize = 4_096;
const MAX_PROOF_AGE_MS: u64 = 60_000;

/// Closed HTTP purpose DATA; selecting a purpose never admits an FI session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryFiHttpProofPurposeV1 {
    /// Exact POST completion under the original enrollment token.
    StagedEnrollmentComplete,
    /// Exact GET status under that original staged token.
    StagedEnrollmentStatus,
    /// Exact POST binding under the original unbound access token.
    DeviceSessionBind,
    /// Exact GET verification of a candidate DPoP token, before product publication.
    CandidateSessionWhoAmI,
    /// Ordinary selected-FI API request, excluding enrollment and authentication routes.
    CurrentHttp,
}
impl KagemushaOrdinaryFiHttpProofPurposeV1 {
    /// Exact authorization scheme; FI still authenticates the offered token DATA.
    #[must_use]
    pub const fn authorization_scheme(self) -> &'static str {
        match self {
            Self::StagedEnrollmentComplete
            | Self::StagedEnrollmentStatus
            | Self::DeviceSessionBind => "Bearer",
            Self::CandidateSessionWhoAmI | Self::CurrentHttp => "DPoP",
        }
    }
    /// Exact purpose-specific proof header name.
    #[must_use]
    pub const fn proof_header_name(self) -> &'static str {
        match self {
            Self::StagedEnrollmentComplete | Self::StagedEnrollmentStatus => "Enrollment-DPoP",
            Self::DeviceSessionBind | Self::CandidateSessionWhoAmI | Self::CurrentHttp => "DPoP",
        }
    }
    fn require_route(self, method: &str, path: &str) -> Result<()> {
        let accepted = match self {
            Self::StagedEnrollmentComplete => {
                method == "POST" && path == "/v1/devices/enrollment/complete"
            }
            Self::StagedEnrollmentStatus => {
                method == "GET" && path == "/v1/devices/enrollment/status"
            }
            Self::DeviceSessionBind => method == "POST" && path == "/v1/auth/device-session",
            Self::CandidateSessionWhoAmI => method == "GET" && path == "/v1/auth/whoami",
            Self::CurrentHttp => {
                let ordinary = matches!(method, "GET" | "POST" | "PUT" | "PATCH" | "DELETE")
                    && path.starts_with("/v1/")
                    && !path.starts_with("/v1/devices/enrollment/")
                    && !path.starts_with("/v1/auth/");
                if ordinary && path.contains('%') {
                    return require_canonical_encoded_fee_account_path(method, path);
                }
                ordinary
            }
        };
        if accepted { Ok(()) } else { Err(Rejected) }
    }
}

/// Exact immutable request DATA, with no platform key, alias, JKT or credential selector.
/// Native must capture it from one final HTTP request/token snapshot and independently selected
/// FI configuration. Full query/body/idempotency originals remain privately held by transport;
/// DPoP does not cryptographically bind those correlation fields.
pub struct KagemushaOrdinaryFiHttpRequestDataV1 {
    /// One closed operation grammar.
    pub purpose: KagemushaOrdinaryFiHttpProofPurposeV1,
    /// Exact uppercase final method.
    pub method: String,
    /// Canonical final HTTPS URI, query/fragment removed and external prefix retained.
    pub htu: String,
    /// Source-selected FI identity, matched to the genuinely held runtime.
    pub fi_id: String,
    /// Source-selected authentication namespace, matched to that runtime.
    pub authentication_namespace: String,
    /// Canonical selected HTTPS origin without trailing slash, credentials or path.
    pub fi_https_origin: String,
    /// Canonical external prefix, empty at root or such as `/api`.
    pub fi_external_path_prefix: String,
    /// Exact confidential token bytes; DATA with no Native authentication grant.
    pub access_token: Zeroizing<Vec<u8>>,
    /// Original canonical lowercase request UUID; correlation DATA only.
    pub original_request_id: String,
    /// Bounded original private operation/epoch reference; correlation DATA only.
    pub operation_reference: Vec<u8>,
    /// Transport's immutable complete request-snapshot hash, not a DPoP-signed body claim.
    pub request_snapshot_sha256: [u8; 32],
}
impl KagemushaOrdinaryFiHttpRequestDataV1 {
    fn validate(&self) -> Result<()> {
        if self.access_token.is_empty()
            || self.access_token.len() > MAX_TOKEN_BYTES
            || !self
                .access_token
                .iter()
                .all(|byte| (0x21..=0x7e).contains(byte))
            || self.fi_id.is_empty()
            || self.fi_id.len() > 128
            || self.authentication_namespace.is_empty()
            || self.authentication_namespace.len() > 128
            || !canonical_request_uuid(&self.original_request_id)
            || self.operation_reference.is_empty()
            || self.operation_reference.len() > MAX_OPERATION_BYTES
            || self.request_snapshot_sha256 == [0; 32]
        {
            return Err(Rejected);
        }
        let path = canonical_selected_api_path(
            &self.htu,
            &self.fi_https_origin,
            &self.fi_external_path_prefix,
        )?;
        self.purpose.require_route(&self.method, path)
    }
}

#[derive(norito::derive::JsonSerialize)]
struct Jwk {
    kty: &'static str,
    crv: &'static str,
    x: String,
    y: String,
}
// RFC7638 ordered fields. Norito's actual writer preserves declaration order.
#[derive(norito::derive::JsonSerialize)]
struct Thumbprint<'a> {
    crv: &'static str,
    kty: &'static str,
    x: &'a str,
    y: &'a str,
}
#[derive(norito::derive::JsonSerialize)]
struct Header {
    typ: &'static str,
    alg: &'static str,
    jwk: Jwk,
}
#[derive(norito::derive::JsonSerialize)]
struct Claims<'a> {
    htm: &'a str,
    htu: &'a str,
    iat: i64,
    jti: &'a str,
    ath: String,
}

enum Stage {
    Prepared,
    PlatformStarted,
    Failed,
    Completed {
        original_der: Vec<u8>,
        compact: Zeroizing<String>,
    },
}

/// Opaque current-key HTTP preparation. No decoder, `Clone`, bytes factory, offered alias or
/// signer callback recreates this holder. Actual finalized identity custody is required.
/// Basic hardware identity signing requires no offline or single-use monetary capability.
pub struct KagemushaPreparedOrdinaryFiHttpProofV1 {
    selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
    pending_scope: [u8; 32],
    possession_ticket: u64,
    credential_digest: [u8; 32],
    point: [u8; 65],
    key_id: [u8; 32],
    original_alias: String,
    attestation_challenge: [u8; 32],
    security_level: KagemushaAppKeySecurityLevelV1,
    request: KagemushaOrdinaryFiHttpRequestDataV1,
    issued_at_ms: u64,
    jti: String,
    jkt: String,
    signing_input: Zeroizing<String>,
    stage: Stage,
}
impl KagemushaPreparedOrdinaryFiHttpProofV1 {
    /// Prepare a fresh bounded proof from actual final identity and independently held scope.
    /// A JWT, FI response DTO or offered account/key tuple supplies none of those originals.
    /// # Errors
    /// Rejects absent/changed credential, foreign runtime, expired interval, malformed request,
    /// unsupported platform or unavailable real randomness.
    pub fn prepare(
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        request: KagemushaOrdinaryFiHttpRequestDataV1,
    ) -> Result<Self> {
        request.validate()?;
        selected.require_prepared_original_scope(pending.preparation())?;
        let interval = selected.trusted_time_interval()?;
        interval.check_both(|now| {
            pending.recheck_retained_originals_at_trusted_time(now)?;
            possession.final_identity(pending, now).map(|_| ())
        })?;
        let credential = possession.final_identity(pending, interval.lower_ms())?;
        let subject = credential.subject();
        let raw = pending.raw_admission().subject();
        let owner = &pending.preparation.owner;
        if subject.platform_class != KagemushaHardwarePlatformClassV1::AndroidKeyMint
            || raw.platform_class != subject.platform_class
            || !matches!(
                subject.security_level,
                KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment
                    | KagemushaAppKeySecurityLevelV1::StrongBox
            )
            || raw.security_level != subject.security_level
            || raw.app_public_key != subject.app_public_key
            || raw.attested_key_id != subject.attested_key_id
            || subject.account_binding
                != kagemusha_ordinary_app_account_binding_v1(&owner.account_id)
            || subject.network_id != *owner.runtime.network_id.as_bytes()
            || subject.lane_id != owner.lane_id
            || request.fi_id != owner.runtime.fi_id.as_ref()
            || request.authentication_namespace != owner.runtime.authentication_namespace.as_ref()
        {
            return Err(Rejected);
        }
        let point = *subject.app_public_key.as_sec1_bytes();
        VerifyingKey::from_sec1_bytes(&point).map_err(|_| Rejected)?;
        if <[u8; 32]>::from(Sha256::digest(point)) != subject.attested_key_id {
            return Err(Rejected);
        }
        let jti = fresh_jti()?;
        let issued_at_ms = interval.lower_ms();
        let iat = i64::try_from(issued_at_ms / 1_000).map_err(|_| Rejected)?;
        let (signing_input, jkt) = encode_signing_input(&point, &request, iat, &jti)?;
        let this = Self {
            selected,
            pending_scope: pending.native_scope(),
            possession_ticket: possession.ticket(),
            credential_digest: credential.digest(),
            point,
            key_id: subject.attested_key_id,
            original_alias: pending.original_alias().into(),
            attestation_challenge: pending
                .preparation
                .preparation
                .challenge
                .attestation_challenge()
                .map_err(|_| Rejected)?,
            security_level: subject.security_level,
            request,
            issued_at_ms,
            jti,
            jkt,
            signing_input: Zeroizing::new(signing_input),
            stage: Stage::Prepared,
        };
        this.recheck(pending, possession)?;
        Ok(this)
    }
    fn recheck(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<()> {
        self.selected
            .require_prepared_original_scope(pending.preparation())?;
        self.request.validate()?;
        let interval = self.selected.trusted_time_interval()?;
        if interval.lower_ms() < self.issued_at_ms
            || interval
                .upper_ms()
                .checked_sub(self.issued_at_ms)
                .ok_or(Custody)?
                >= MAX_PROOF_AGE_MS
            || pending.native_scope() != self.pending_scope
            || possession.ticket() != self.possession_ticket
            || pending.original_alias() != self.original_alias
            || self.request.fi_id != pending.preparation.owner.runtime.fi_id.as_ref()
            || self.request.authentication_namespace
                != pending
                    .preparation
                    .owner
                    .runtime
                    .authentication_namespace
                    .as_ref()
        {
            return Err(Custody);
        }
        interval.check_both(|now| {
            pending.recheck_retained_originals_at_trusted_time(now)?;
            let credential = possession.final_identity(pending, now)?;
            let subject = credential.subject();
            if credential.digest() != self.credential_digest
                || subject.app_public_key.as_sec1_bytes() != &self.point
                || subject.attested_key_id != self.key_id
                || subject.security_level != self.security_level
            {
                return Err(Custody);
            }
            Ok(())
        })
    }
    /// Lend one exact original-key/input operation, marking possible invocation before projection.
    /// Native must already hold its authentic current retirement and original platform resources.
    /// # Errors
    /// Rejects stale custody or any previous possible invocation, including uncertain failure.
    pub fn begin_platform<'a>(
        &'a mut self,
        pending: &'a KagemushaPendingAppIdentityV1,
        possession: &'a KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<KagemushaOrdinaryFiHttpProofKeyLoanV1<'a>> {
        self.recheck(pending, possession)?;
        if !matches!(self.stage, Stage::Prepared) {
            return Err(KagemushaOrdinaryIdentityErrorV1::UnknownOutcome);
        }
        self.stage = Stage::PlatformStarted;
        Ok(KagemushaOrdinaryFiHttpProofKeyLoanV1 {
            prepared: self,
            pending,
            possession,
        })
    }
    /// Verify exact original canonical Android DER against the held point/input and return
    /// compact ES256 DATA. Original r/s is preserved, without normalization/replacement.
    /// Repeated completion only reads that same result; it authorizes no signing or dispatch.
    /// # Errors
    /// Rejects another DER/input/key or stale custody. Failed completion stays frozen.
    pub fn complete_original<'a>(
        &'a mut self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        original_der: &[u8],
    ) -> Result<&'a str> {
        self.recheck(pending, possession)?;
        if matches!(self.stage, Stage::Completed { .. }) {
            return match &self.stage {
                Stage::Completed {
                    original_der: retained,
                    compact,
                } if retained.as_slice() == original_der => Ok(compact.as_str()),
                _ => Err(Rejected),
            };
        }
        if !matches!(self.stage, Stage::PlatformStarted) {
            return Err(KagemushaOrdinaryIdentityErrorV1::UnknownOutcome);
        }
        self.stage = Stage::Failed;
        let signature =
            verify_original_der(&self.point, self.signing_input.as_bytes(), original_der)?;
        let compact = format!(
            "{}.{}",
            self.signing_input.as_str(),
            URL_SAFE_NO_PAD.encode(signature)
        );
        if compact.len() > MAX_PROOF_BYTES {
            return Err(Rejected);
        }
        self.recheck(pending, possession)?;
        self.stage = Stage::Completed {
            original_der: original_der.to_vec(),
            compact: Zeroizing::new(compact),
        };
        match &self.stage {
            Stage::Completed { compact, .. } => Ok(compact.as_str()),
            _ => Err(Custody),
        }
    }
    /// Borrow the exact captured request/token DATA for comparison with Native's private transport snapshot.
    /// # Errors
    /// Rejects current identity or interval drift.
    pub fn request_data<'a>(
        &'a self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<&'a KagemushaOrdinaryFiHttpRequestDataV1> {
        self.recheck(pending, possession)?;
        Ok(&self.request)
    }
    /// Borrow Core-derived same-key JKT correlation DATA, granting no FI admission.
    /// # Errors
    /// Rejects current identity or interval drift.
    pub fn jkt<'a>(
        &'a self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<&'a str> {
        self.recheck(pending, possession)?;
        Ok(&self.jkt)
    }
    /// Borrow independently random per-proof UUID correlation DATA.
    /// # Errors
    /// Rejects current identity or interval drift.
    pub fn jti<'a>(
        &'a self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<&'a str> {
        self.recheck(pending, possession)?;
        Ok(&self.jti)
    }
}

/// Exact borrowed platform-key/input loan, retaining actual pending and possession owners.
/// Fields and construction are private; no caller alias resolver or signer callback enters Core.
pub struct KagemushaOrdinaryFiHttpProofKeyLoanV1<'a> {
    prepared: &'a KagemushaPreparedOrdinaryFiHttpProofV1,
    pending: &'a KagemushaPendingAppIdentityV1,
    possession: &'a KagemushaOrdinaryAppPossessionAttemptV1,
}
impl KagemushaOrdinaryFiHttpProofKeyLoanV1<'_> {
    fn recheck(&self) -> Result<()> {
        self.prepared.recheck(self.pending, self.possession)?;
        if !matches!(self.prepared.stage, Stage::PlatformStarted) {
            return Err(Custody);
        }
        Ok(())
    }
    /// Exact Core-generated ASCII JOSE input for one Android SHA256withECDSA operation.
    /// # Errors
    /// Rejects stale held originals.
    pub fn signing_input(&self) -> Result<&[u8]> {
        self.recheck()?;
        Ok(self.prepared.signing_input.as_bytes())
    }
    /// Actual native-selected original generation alias, never caller chosen.
    /// # Errors
    /// Rejects stale held originals.
    pub fn original_alias(&self) -> Result<&str> {
        self.recheck()?;
        Ok(&self.prepared.original_alias)
    }
    /// Actual verified uncompressed P256 point.
    /// # Errors
    /// Rejects stale held originals.
    pub fn public_key(&self) -> Result<&[u8; 65]> {
        self.recheck()?;
        Ok(&self.prepared.point)
    }
    /// Exact SHA256 of that point for current platform-key correlation.
    /// # Errors
    /// Rejects stale held originals.
    pub fn attested_key_id(&self) -> Result<[u8; 32]> {
        self.recheck()?;
        Ok(self.prepared.key_id)
    }
    /// Exact original attestation challenge for original certificate correlation.
    /// # Errors
    /// Rejects stale held originals.
    pub fn attestation_challenge(&self) -> Result<[u8; 32]> {
        self.recheck()?;
        Ok(self.prepared.attestation_challenge)
    }
    /// Authentic TEE/StrongBox level, with no hardware one-use assertion.
    /// # Errors
    /// Rejects stale held originals.
    pub fn security_level(&self) -> Result<KagemushaAppKeySecurityLevelV1> {
        self.recheck()?;
        Ok(self.prepared.security_level)
    }
}

fn canonical_request_uuid(value: &str) -> bool {
    value.len() == 36
        && value != "00000000-0000-0000-0000-000000000000"
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}
fn fresh_jti() -> Result<String> {
    let mut original = [0_u8; 16];
    OsRng.try_fill_bytes(&mut original).map_err(|_| Custody)?;
    original[6] = (original[6] & 0x0f) | 0x40;
    original[8] = (original[8] & 0x3f) | 0x80;
    let encoded = hex::encode(original);
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &encoded[..8],
        &encoded[8..12],
        &encoded[12..16],
        &encoded[16..20],
        &encoded[20..]
    ))
}
fn canonical_selected_api_path<'a>(htu: &'a str, origin: &str, prefix: &str) -> Result<&'a str> {
    if htu.is_empty()
        || htu.len() > MAX_HTU_BYTES
        || origin.len() > MAX_HTU_BYTES
        || prefix.len() > 256
        || !htu.is_ascii()
        || (!prefix.is_empty()
            && (!prefix.starts_with('/')
                || prefix.ends_with('/')
                || prefix
                    .split('/')
                    .skip(1)
                    .any(|part| part.is_empty() || part == "." || part == "..")
                || !prefix.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~')
                })))
    {
        return Err(Rejected);
    }
    let selected = Url::parse(origin).map_err(|_| Rejected)?;
    if selected.scheme() != "https"
        || selected.host_str().is_none()
        || !selected.username().is_empty()
        || selected.password().is_some()
        || selected.query().is_some()
        || selected.fragment().is_some()
        || selected.path() != "/"
        || selected.origin().ascii_serialization() != origin
    {
        return Err(Rejected);
    }
    let final_uri = Url::parse(htu).map_err(|_| Rejected)?;
    if final_uri.scheme() != "https"
        || final_uri.origin() != selected.origin()
        || !final_uri.username().is_empty()
        || final_uri.password().is_some()
        || final_uri.query().is_some()
        || final_uri.fragment().is_some()
        || final_uri.as_str() != htu
        || final_uri.path().ends_with('/')
    {
        return Err(Rejected);
    }
    let relative = htu
        .strip_prefix(origin)
        .and_then(|path| path.strip_prefix(prefix))
        .ok_or(Rejected)?;
    if !relative.starts_with("/v1/") {
        return Err(Rejected);
    }
    Ok(relative)
}
// Actual Android fee routes encode one canonical I105 account with UTF8 URLEncoder.
// A percent-bearing path gets no generic route authority: only these existing exact routes.
fn require_canonical_encoded_fee_account_path(method: &str, path: &str) -> Result<()> {
    let (encoded, suffix) = path
        .strip_prefix("/v1/validation-fee/accounts/")
        .and_then(|rest| rest.split_once('/'))
        .ok_or(Rejected)?;
    if !matches!(
        (method, suffix),
        ("GET", "status") | ("GET", "statement/head") | ("POST", "statement")
    ) || encoded.is_empty()
    {
        return Err(Rejected);
    }
    // Canonical I105's raw ASCII is alphanumeric. Escapes must be uppercase, non-ASCII UTF8
    // bytes only; reject encoded separators/dots/controls/percent and double-encoded aliases.
    let bytes = encoded.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_alphanumeric() {
            index += 1;
            continue;
        }
        if bytes[index] != b'%' || bytes.len() - index < 3 {
            return Err(Rejected);
        }
        let high = uppercase_hex_digit(bytes[index + 1]).ok_or(Rejected)?;
        let low = uppercase_hex_digit(bytes[index + 2]).ok_or(Rejected)?;
        if (high << 4 | low) < 0x80 {
            return Err(Rejected);
        }
        index += 3;
    }
    // Reuse the actual URL crate's public form codec matching the product's URLEncoder.
    // Its lossy UTF8 cannot be admitted: exact re-encode plus genuine canonical Model account
    // parsing must both agree, rejecting replacement/overlong/invalid encodings and aliases.
    let mut decoded = url::form_urlencoded::parse(bytes);
    let (literal, value) = decoded.next().ok_or(Rejected)?;
    if !value.is_empty()
        || decoded.next().is_some()
        || url::form_urlencoded::byte_serialize(literal.as_bytes()).collect::<String>() != encoded
    {
        return Err(Rejected);
    }
    let account = AccountId::parse_encoded(literal.as_ref()).map_err(|_| Rejected)?;
    if account.canonical_i105().map_err(|_| Rejected)? != literal.as_ref() {
        return Err(Rejected);
    }
    Ok(())
}
fn uppercase_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
fn encode_signing_input(
    point: &[u8; 65],
    request: &KagemushaOrdinaryFiHttpRequestDataV1,
    iat: i64,
    jti: &str,
) -> Result<(String, String)> {
    VerifyingKey::from_sec1_bytes(point).map_err(|_| Rejected)?;
    let jwk = Jwk {
        kty: "EC",
        crv: "P-256",
        x: URL_SAFE_NO_PAD.encode(&point[1..33]),
        y: URL_SAFE_NO_PAD.encode(&point[33..65]),
    };
    let thumbprint = norito::json::to_vec(&Thumbprint {
        crv: jwk.crv,
        kty: jwk.kty,
        x: &jwk.x,
        y: &jwk.y,
    })
    .map_err(|_| Rejected)?;
    let jkt = URL_SAFE_NO_PAD.encode(Sha256::digest(thumbprint));
    let header = norito::json::to_vec(&Header {
        typ: "dpop+jwt",
        alg: "ES256",
        jwk,
    })
    .map_err(|_| Rejected)?;
    let claims = norito::json::to_vec(&Claims {
        htm: &request.method,
        htu: &request.htu,
        iat,
        jti,
        ath: URL_SAFE_NO_PAD.encode(Sha256::digest(request.access_token.as_slice())),
    })
    .map_err(|_| Rejected)?;
    let input = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header),
        URL_SAFE_NO_PAD.encode(claims)
    );
    if input.len().checked_add(87).ok_or(Rejected)? > MAX_PROOF_BYTES {
        return Err(Rejected);
    }
    Ok((input, jkt))
}
fn verify_original_der(point: &[u8; 65], input: &[u8], original: &[u8]) -> Result<[u8; 64]> {
    if !(8..=72).contains(&original.len()) {
        return Err(Rejected);
    }
    let signature = Signature::from_der(original).map_err(|_| Rejected)?;
    if signature.to_der().as_bytes() != original {
        return Err(Rejected);
    }
    VerifyingKey::from_sec1_bytes(point)
        .map_err(|_| Rejected)?
        .verify(input, &signature)
        .map_err(|_| Rejected)?;
    Ok(signature.to_bytes().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{SigningKey, signature::Signer as _};
    fn request(
        purpose: KagemushaOrdinaryFiHttpProofPurposeV1,
        method: &str,
        path: &str,
    ) -> KagemushaOrdinaryFiHttpRequestDataV1 {
        KagemushaOrdinaryFiHttpRequestDataV1 {
            purpose,
            method: method.into(),
            htu: format!("https://fi.example/api{path}"),
            fi_id: "test-fi".into(),
            authentication_namespace: "test-auth".into(),
            fi_https_origin: "https://fi.example".into(),
            fi_external_path_prefix: "/api".into(),
            access_token: Zeroizing::new(b"opaque.token.DATA".to_vec()),
            original_request_id: "b1214662-9a3a-4d5c-80ca-54f89b9c57ca".into(),
            operation_reference: b"private-operation-1".to_vec(),
            request_snapshot_sha256: [1; 32],
        }
    }
    #[test]
    fn closed_purposes_and_final_origin_refuse_substitution() {
        use KagemushaOrdinaryFiHttpProofPurposeV1::*;
        for (purpose, method, path) in [
            (
                StagedEnrollmentComplete,
                "POST",
                "/v1/devices/enrollment/complete",
            ),
            (
                StagedEnrollmentStatus,
                "GET",
                "/v1/devices/enrollment/status",
            ),
            (DeviceSessionBind, "POST", "/v1/auth/device-session"),
            (CandidateSessionWhoAmI, "GET", "/v1/auth/whoami"),
            (CurrentHttp, "POST", "/v1/payments"),
        ] {
            assert!(request(purpose, method, path).validate().is_ok());
        }
        assert!(
            request(CurrentHttp, "POST", "/v1/auth/device-session")
                .validate()
                .is_err()
        );
        assert!(
            request(CurrentHttp, "POST", "/v1/devices/enrollment/complete")
                .validate()
                .is_err()
        );
        assert!(
            request(DeviceSessionBind, "GET", "/v1/auth/device-session")
                .validate()
                .is_err()
        );
        let mut tilde = request(CandidateSessionWhoAmI, "GET", "/v1/auth/whoami");
        tilde.fi_external_path_prefix = "/tenant~1".into();
        tilde.htu = "https://fi.example/tenant~1/v1/auth/whoami".into();
        assert!(tilde.validate().is_ok());
        for htu in [
            "http://fi.example/api/v1/auth/whoami",
            "https://other.example/api/v1/auth/whoami",
            "https://fi.example/api2/v1/auth/whoami",
            "https://fi.example/api/v1/auth/whoami?token=x",
            "https://fi.example/api/v1/auth/whoami#x",
            "https://x@fi.example/api/v1/auth/whoami",
            "https://fi.example:443/api/v1/auth/whoami",
            "https://fi.example/api/v1/auth/./whoami",
            "https://fi.example/api/v1/auth/%77hoami",
            "https://fi.example/api/v1/auth/whoami/",
        ] {
            let mut offered = request(CandidateSessionWhoAmI, "GET", "/v1/auth/whoami");
            offered.htu = htu.into();
            assert!(offered.validate().is_err(), "{htu}");
        }
        let mut offered = request(CandidateSessionWhoAmI, "GET", "/v1/auth/whoami");
        offered.access_token = Zeroizing::new(b"token DATA".to_vec());
        assert!(offered.validate().is_err());
        offered.access_token = Zeroizing::new(vec![b'a'; MAX_TOKEN_BYTES + 1]);
        assert!(offered.validate().is_err());
        offered.access_token = Zeroizing::new(b"DATA".to_vec());
        offered.original_request_id = "B1214662-9a3a-4d5c-80ca-54f89b9c57ca".into();
        assert!(offered.validate().is_err());
    }
    #[test]
    fn current_fee_account_routes_keep_exact_utf8_i105_segments() {
        let _chain = iroha_data_model::account::address::ChainDiscriminantGuard::enter(0x02F1);
        use KagemushaOrdinaryFiHttpProofPurposeV1::*;
        // Existing Model test PUBLIC key, not an attestation/issuer/private key or authority grant.
        let key = iroha_crypto::PublicKey::from_hex(
            iroha_crypto::Algorithm::Ed25519,
            "27c96646f2d4632d4fc241f84cbc427fbc3ecaa95becba55088d6c7b81fc5bbf",
        )
        .unwrap();
        let literal = AccountId::new(key).canonical_i105().unwrap();
        let encoded = url::form_urlencoded::byte_serialize(literal.as_bytes()).collect::<String>();
        assert!(
            encoded.contains('%'),
            "actual I105 kana must use UTF8 escapes"
        );
        for (method, suffix) in [
            ("GET", "status"),
            ("GET", "statement/head"),
            ("POST", "statement"),
        ] {
            let path = format!("/v1/validation-fee/accounts/{encoded}/{suffix}");
            assert!(request(CurrentHttp, method, &path).validate().is_ok());
            assert!(
                request(DeviceSessionBind, method, &path)
                    .validate()
                    .is_err()
            );
        }
        for bad in [
            format!("{encoded}%2F"),
            format!("{encoded}%5C"),
            format!("{encoded}%2E"),
            format!("{encoded}%252F"),
            format!("{encoded}%20"),
            format!("{encoded}%00"),
            format!("{encoded}%FF"),
            format!("{encoded}%"),
            format!("{encoded}%GG"),
            encoded.to_ascii_lowercase(),
            format!("{encoded}+"),
            "alias%EF%BD%B2".into(),
        ] {
            let path = format!("/v1/validation-fee/accounts/{bad}/status");
            assert!(request(CurrentHttp, "GET", &path).validate().is_err());
        }
        assert!(
            request(
                CurrentHttp,
                "POST",
                &format!("/v1/validation-fee/accounts/{encoded}/status")
            )
            .validate()
            .is_err()
        );
        assert!(
            request(
                CurrentHttp,
                "GET",
                &format!("/v1/validation-fee/accounts/{encoded}/statement")
            )
            .validate()
            .is_err()
        );
        assert!(
            request(CurrentHttp, "GET", "/v1/auth/%77hoami")
                .validate()
                .is_err()
        );
        assert!(
            request(CandidateSessionWhoAmI, "GET", "/v1/auth/%77hoami")
                .validate()
                .is_err()
        );
        assert!(
            request(CurrentHttp, "POST", "/v1/devices/enrollment/%63omplete")
                .validate()
                .is_err()
        );
    }
    #[test]
    fn purpose_headers_and_fresh_native_uuid_are_closed() {
        use KagemushaOrdinaryFiHttpProofPurposeV1::*;
        for purpose in [StagedEnrollmentComplete, StagedEnrollmentStatus] {
            assert_eq!(purpose.authorization_scheme(), "Bearer");
            assert_eq!(purpose.proof_header_name(), "Enrollment-DPoP");
        }
        assert_eq!(DeviceSessionBind.authorization_scheme(), "Bearer");
        assert_eq!(DeviceSessionBind.proof_header_name(), "DPoP");
        for purpose in [CandidateSessionWhoAmI, CurrentHttp] {
            assert_eq!(purpose.authorization_scheme(), "DPoP");
            assert_eq!(purpose.proof_header_name(), "DPoP");
        }
        let jti = fresh_jti().expect("real OS randomness");
        assert!(canonical_request_uuid(&jti));
        assert_eq!(jti.as_bytes()[14], b'4');
        assert!(matches!(jti.as_bytes()[19], b'8' | b'9' | b'a' | b'b'));
    }
    #[test]
    fn original_der_binds_exact_jose_token_and_held_point() {
        // Public test-only scalar; no fake attestation, issuer or enrolled owner is constructed.
        let signing = SigningKey::from_slice(&[7; 32]).expect("test scalar");
        let point: [u8; 65] = signing
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .try_into()
            .unwrap();
        let original = request(
            KagemushaOrdinaryFiHttpProofPurposeV1::DeviceSessionBind,
            "POST",
            "/v1/auth/device-session",
        );
        let jti = "50cf70d8-45d0-4186-87c5-5e9c3e097c08";
        let (input, jkt) = encode_signing_input(&point, &original, 1_800_000_000, jti).unwrap();
        let signature: Signature = signing.sign(input.as_bytes());
        let der = signature.to_der();
        let raw: [u8; 64] = signature.to_bytes().into();
        assert_eq!(
            verify_original_der(&point, input.as_bytes(), der.as_bytes()).unwrap(),
            raw
        );
        let mut changed = request(
            KagemushaOrdinaryFiHttpProofPurposeV1::DeviceSessionBind,
            "POST",
            "/v1/auth/device-session",
        );
        changed.access_token = Zeroizing::new(b"other.exact.token".to_vec());
        let (different, _) = encode_signing_input(&point, &changed, 1_800_000_000, jti).unwrap();
        assert!(verify_original_der(&point, different.as_bytes(), der.as_bytes()).is_err());
        let other = SigningKey::from_slice(&[8; 32]).unwrap();
        let other_point = other.verifying_key().to_encoded_point(false);
        assert!(
            verify_original_der(
                other_point.as_bytes().try_into().unwrap(),
                input.as_bytes(),
                der.as_bytes()
            )
            .is_err()
        );
        let mut trailing = der.as_bytes().to_vec();
        trailing.push(0);
        assert!(verify_original_der(&point, input.as_bytes(), &trailing).is_err());
        let x = URL_SAFE_NO_PAD.encode(&point[1..33]);
        let y = URL_SAFE_NO_PAD.encode(&point[33..65]);
        let thumbprint = norito::json::to_string(&Thumbprint {
            crv: "P-256",
            kty: "EC",
            x: &x,
            y: &y,
        })
        .unwrap();
        assert_eq!(
            thumbprint,
            format!("{{\"crv\":\"P-256\",\"kty\":\"EC\",\"x\":\"{x}\",\"y\":\"{y}\"}}")
        );
        assert_eq!(
            jkt,
            URL_SAFE_NO_PAD.encode(Sha256::digest(thumbprint.as_bytes()))
        );
        let (header, claims) = input.split_once('.').unwrap();
        let header = String::from_utf8(URL_SAFE_NO_PAD.decode(header).unwrap()).unwrap();
        assert!(header.starts_with("{\"typ\":\"dpop+jwt\",\"alg\":\"ES256\",\"jwk\":{"));
        let claims = String::from_utf8(URL_SAFE_NO_PAD.decode(claims).unwrap()).unwrap();
        let ath = URL_SAFE_NO_PAD.encode(Sha256::digest(original.access_token.as_slice()));
        assert_eq!(
            claims,
            format!(
                "{{\"htm\":\"POST\",\"htu\":\"https://fi.example/api/v1/auth/device-session\",\"iat\":1800000000,\"jti\":\"{jti}\",\"ath\":\"{ath}\"}}"
            )
        );
    }
}
