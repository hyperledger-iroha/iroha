//! Purpose-specific ordinary enrollment HTTP originals. This codec verifies an exact
//! signature, not a current FI customer, certified wallet read, issuer or installed runtime.
//! The receiver supplies its own current network/FI/actor/operation/HTTPS target. Offered
//! Host, Forwarded, Torii auth headers and DTOs cannot select any of those owners.

use super::{ParticipantEnrollmentOperationV1, ParticipantEnrollmentRequestV1};
use eyre::{Result, ensure};
use iroha_crypto::Signature;
use iroha_data_model::{NetworkId, account::AccountId};
use sha2::{Digest as _, Sha256};
use url::Url;

/// The sole first-release HTTP metadata names; Authorization is captured separately.
/// These are purpose-specific carriers, not Torii request-signature headers.
pub const PARTICIPANT_ENROLLMENT_HTTP_HEADER_NAMES: [&str; 8] = [
    "x-iroha-enrollment-contract",
    "x-iroha-enrollment-signatory",
    "x-iroha-enrollment-wallet",
    "x-iroha-enrollment-request-id",
    "idempotency-key",
    "x-iroha-enrollment-timestamp",
    "x-iroha-enrollment-nonce",
    "x-iroha-enrollment-signature",
];

/// Independently selected receiver context, obtained from the actual authenticated FI
/// session, handler and configured external mount before parsing offered request headers.
/// These primitive references establish no FI authority by themselves.
#[derive(Clone, Copy)]
pub struct ParticipantEnrollmentHttpOwnerContextV1<'a> {
    /// Exact admitted network; never supplied by an offered header.
    pub network_id: &'a NetworkId,
    /// Exact FI authentication namespace, distinct from the Torii dataspace header.
    pub authentication_namespace: &'a str,
    /// Current FI-local customer actor from the authenticated session.
    pub actor_id: &'a str,
    /// Actual handler purpose, not a caller-provided route selector.
    pub operation: ParticipantEnrollmentOperationV1,
    /// Actual HTTP method from the receiving request; only exact POST is admitted.
    pub http_method: &'a str,
    /// Actual untouched origin-form path/query from the receiving URI, before any rewrite.
    pub request_target: &'a str,
    /// Complete pinned external HTTPS target, including the independently configured mount.
    pub target: &'a Url,
}

/// Untrusted HTTP carrier whose real Ed signature has been checked against exact bytes.
/// There is no decoder for `VerifiedParticipantEnrollmentRequestV1`: the actual receiver
/// must still fetch its own current certified S/W cut and perform FI admission before CAS.
pub struct ParticipantEnrollmentHttpReceivedOriginalV1<'a> {
    owner: ParticipantEnrollmentHttpOwnerContextV1<'a>,
    original_body: &'a [u8],
    session_sha256: [u8; 32],
    signatory: AccountId,
    wallet: AccountId,
    request_id: String,
    idempotency_key: String,
    timestamp_ms: u64,
    nonce: String,
    signature: Signature,
}
impl ParticipantEnrollmentHttpReceivedOriginalV1<'_> {
    /// Borrow the exact original body and signed context for the real receiving producer.
    #[must_use]
    pub fn request(&self) -> ParticipantEnrollmentRequestV1<'_> {
        ParticipantEnrollmentRequestV1 {
            network_id: self.owner.network_id,
            authentication_namespace: self.owner.authentication_namespace,
            actor_id: self.owner.actor_id,
            session_sha256: self.session_sha256,
            signatory: &self.signatory,
            wallet: &self.wallet,
            request_id: &self.request_id,
            idempotency_key: &self.idempotency_key,
            operation: self.owner.operation,
            target: self.owner.target,
            body: self.original_body,
            timestamp_ms: self.timestamp_ms,
            nonce: &self.nonce,
        }
    }
    /// Exact retained original signature; this is not a current wallet/Native capability.
    #[must_use]
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
}

fn authorization_digest(raw: &[u8]) -> Result<[u8; 32]> {
    // No trim, case conversion, scheme replacement or JWT serialization is permitted.
    // Actual bearer authentication remains the FI handler's prerequisite.
    ensure!(
        !raw.is_empty() && raw.len() <= 8192 && raw.iter().all(|b| (0x20..=0x7e).contains(b)),
        "invalid exact enrollment Authorization original"
    );
    Ok(Sha256::digest(raw).into())
}

/// Encode only the exact purpose-specific original and its real existing signature.
/// Authorization stays runtime-only, byte-identical to the session bound by the signer;
/// callers must never persist or log this returned transport metadata.
/// This pure codec grants no Native ownership. Use the Native signed holder for dispatch.
/// # Errors
/// Refuses a foreign session, wrong signer, malformed subject or non-Ed64 signature.
pub fn encode_participant_enrollment_http_headers_v1(
    request: &ParticipantEnrollmentRequestV1<'_>,
    signature: &Signature,
    authorization: &[u8],
) -> Result<Vec<(&'static str, Vec<u8>)>> {
    ensure!(
        authorization_digest(authorization)? == request.session_sha256,
        "enrollment transport changed exact Authorization session"
    );
    canonical_nonce(request.nonce)?;
    let message = request.signing_message()?;
    let key = request
        .signatory
        .try_signatory()
        .ok_or_else(|| eyre::eyre!("missing enrollment S"))?;
    signature.verify(key, &message)?;
    ensure!(
        signature.payload().len() == 64,
        "enrollment HTTP signature is not Ed64"
    );
    let values = [
        "1".to_owned(),
        request.signatory.canonical_i105()?,
        request.wallet.canonical_i105()?,
        request.request_id.to_owned(),
        request.idempotency_key.to_owned(),
        request.timestamp_ms.to_string(),
        request.nonce.to_owned(),
        hex::encode(signature.payload()),
    ];
    let mut out = PARTICIPANT_ENROLLMENT_HTTP_HEADER_NAMES
        .into_iter()
        .zip(values)
        .map(|(name, value)| (name, value.into_bytes()))
        .collect::<Vec<_>>();
    out.push(("authorization", authorization.to_vec()));
    Ok(out)
}

fn canonical_nonce(raw: &str) -> Result<()> {
    ensure!(
        raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "noncanonical Native enrollment nonce header"
    );
    Ok(())
}

fn canonical_decimal(raw: &str) -> Result<u64> {
    ensure!(
        !raw.is_empty()
            && raw.len() <= 20
            && raw.bytes().all(|b| b.is_ascii_digit())
            && (raw.len() == 1 || !raw.starts_with('0')),
        "noncanonical enrollment timestamp"
    );
    let value = raw.parse::<u64>()?;
    ensure!(
        value.to_string() == raw,
        "noncanonical enrollment timestamp"
    );
    Ok(value)
}
fn canonical_account(raw: &str) -> Result<AccountId> {
    ensure!(
        !raw.is_empty() && raw.len() <= 4096,
        "unbounded enrollment account header"
    );
    let account = AccountId::parse_encoded(raw)?;
    ensure!(
        account.canonical_i105()? == raw,
        "noncanonical enrollment account header"
    );
    Ok(account)
}

/// Decode exact, flattened HTTP header values and verify the real original Ed signature.
/// Pass every value from the actual `HeaderMap`. Duplicate identical values are rejected;
/// this must run before a generic `.get()` helper could hide duplicate Authorization.
/// Unknown names in the purpose prefix are rejected. Other HTTP headers select no owner.
/// # Errors
/// Missing/duplicate/noncanonical fields, changed original context/body/session or signature.
pub fn decode_participant_enrollment_http_original_v1<'a, 'h>(
    owner: ParticipantEnrollmentHttpOwnerContextV1<'a>,
    original_body: &'a [u8],
    headers: impl IntoIterator<Item = (&'h str, &'h [u8])>,
) -> Result<ParticipantEnrollmentHttpReceivedOriginalV1<'a>> {
    ensure!(
        owner.http_method == "POST"
            && owner.request_target == owner.target.path()
            && owner.target.query().is_none(),
        "enrollment actual HTTP method/target differs from pinned mount"
    );
    let mut values: [Option<&[u8]>; 9] = [None; 9];
    for (name, value) in headers {
        // HeaderMap names are ASCII case-insensitive. Accept their actual spelling, but do
        // not apply that normalization to any field value or signed Authorization bytes.
        let index = if name.eq_ignore_ascii_case("authorization") {
            Some(8)
        } else {
            PARTICIPANT_ENROLLMENT_HTTP_HEADER_NAMES
                .iter()
                .position(|n| name.eq_ignore_ascii_case(n))
        };
        if let Some(index) = index {
            ensure!(
                values[index].is_none(),
                "duplicate exact enrollment HTTP field"
            );
            ensure!(value.len() <= 8192, "unbounded enrollment HTTP field");
            values[index] = Some(value);
        } else {
            ensure!(
                !name.to_ascii_lowercase().starts_with("x-iroha-enrollment-"),
                "unknown first-release enrollment HTTP field"
            );
        }
    }
    let value = |index: usize| -> Result<&str> {
        Ok(std::str::from_utf8(values[index].ok_or_else(|| {
            eyre::eyre!("missing exact enrollment HTTP field")
        })?)?)
    };
    ensure!(value(0)? == "1", "unsupported enrollment HTTP contract");
    let signature_literal = value(7)?;
    ensure!(
        signature_literal.len() == 128
            && signature_literal
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "noncanonical enrollment Ed64 header"
    );
    canonical_nonce(value(6)?)?;
    let raw_signature = hex::decode(signature_literal)?;
    let original = ParticipantEnrollmentHttpReceivedOriginalV1 {
        owner,
        original_body,
        session_sha256: authorization_digest(
            values[8].ok_or_else(|| eyre::eyre!("missing Authorization original"))?,
        )?,
        signatory: canonical_account(value(1)?)?,
        wallet: canonical_account(value(2)?)?,
        request_id: value(3)?.into(),
        idempotency_key: value(4)?.into(),
        timestamp_ms: canonical_decimal(value(5)?)?,
        nonce: value(6)?.into(),
        signature: Signature::from_bytes(&raw_signature),
    };
    let request = original.request();
    let message = request.signing_message()?;
    let key = request
        .signatory
        .try_signatory()
        .ok_or_else(|| eyre::eyre!("missing enrollment S"))?;
    original.signature.verify(key, &message)?;
    Ok(original)
}
