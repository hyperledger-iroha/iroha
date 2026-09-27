//! Exact issuer-signed app preparation for the one-use native enrollment ceremony.
//!
//! Only independently pinned native selection and policy values may be passed as `pins`.
//! This verifier is pure: it neither opens a wallet nor admits a monetary credential.

use iroha_crypto::{Algorithm, Signature};
use iroha_data_model::{
    account::AccountId,
    kagemusha::{KagemushaHardwarePlatformClassV1, KagemushaRetailEnrollmentIssuerPolicyV1},
};
use sha2::{Digest as _, Sha256};

const DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-enrollment-preparation\0";
const VERSION: u8 = 1;
const UNSIGNED_BYTES: usize = 1 + 8 + 8 + 6 * 32;
const TOKEN_BYTES: usize = UNSIGNED_BYTES + 64;
const CHALLENGE_TTL_MS: u64 = 120_000;

/// A closed failure of exact signed-preparation verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignedAppPreparationErrorV1 {
    /// Frame length, version or account encoding is invalid.
    Encoding,
    /// A signed value differs from a native pin or trusted time interval.
    Binding,
    /// Issuer policy or Ed25519 signature is invalid.
    Authority,
}

type Result<T> = std::result::Result<T, SignedAppPreparationErrorV1>;

/// Independently selected values retained by the native enrollment owner.
pub struct SignedAppPreparationPinsV1<'a> {
    /// Governing issuer policy; never selected by the preparation or app.
    pub policy: &'a KagemushaRetailEnrollmentIssuerPolicyV1,
    /// Account selected before this one-use challenge.
    pub account_id: &'a AccountId,
    /// Governed platform class of the selected hardware profile.
    pub platform_class: KagemushaHardwarePlatformClassV1,
    /// Provisional App Attest key ID retained before raw attestation; Android uses zero.
    /// A later signed certificate and governed qualification must bind this selection.
    pub selected_attested_key_id: [u8; 32],
    /// Nonzero native nonce retained at selection.
    pub client_nonce: [u8; 32],
    /// Release ID from the authenticated release manifest.
    pub release_id: [u8; 32],
    /// Hardware profile ID from that release.
    pub profile_id: [u8; 32],
    /// Native-generated lane ID retained at selection.
    pub lane_id: [u8; 32],
    /// Trusted service time, never a token or handset clock.
    pub trusted_now_ms: u64,
}

/// The six signed fields and validity interval after exact verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedSignedAppPreparationV1 {
    /// Inclusive issuer issuance time.
    pub issued_at_ms: u64,
    /// Exclusive expiry.
    pub expires_at_ms: u64,
    /// Native-selected client nonce.
    pub client_nonce: [u8; 32],
    /// Fresh issuer nonce used by the platform app challenge.
    pub server_nonce: [u8; 32],
    /// Authenticated release ID.
    pub release_id: [u8; 32],
    /// Governed hardware profile ID.
    pub profile_id: [u8; 32],
    /// Apple App Attest key ID or Android zero sentinel.
    pub attested_key_id: [u8; 32],
    /// Native-selected lane ID.
    pub lane_id: [u8; 32],
}

/// Verify the exact 273-byte issuer frame against native pins and trusted time.
///
/// The Apple key ID is selected before attestation and becomes authoritative only
/// after a signed verifier certificate binds it to the attested point and a later
/// governed qualification. Android's preparatory key ID is exactly zero because
/// KeyMint creates its key after this challenge. This verifier grants no authority.
pub fn verify_signed_app_preparation_v1(
    token: &[u8],
    pins: SignedAppPreparationPinsV1<'_>,
) -> Result<VerifiedSignedAppPreparationV1> {
    if token.len() != TOKEN_BYTES || token[0] != VERSION {
        return Err(SignedAppPreparationErrorV1::Encoding);
    }
    pins.policy
        .validate()
        .map_err(|_| SignedAppPreparationErrorV1::Authority)?;
    if pins
        .account_id
        .try_signatory()
        .is_none_or(|key| key.algorithm() != Algorithm::Ed25519)
        || pins.client_nonce == [0; 32]
        || pins.release_id == [0; 32]
        || pins.profile_id == [0; 32]
        || pins.lane_id == [0; 32]
    {
        return Err(SignedAppPreparationErrorV1::Binding);
    }
    let expected_key_id = match pins.platform_class {
        KagemushaHardwarePlatformClassV1::AppleAppAttest
            if pins.selected_attested_key_id != [0; 32] =>
        {
            pins.selected_attested_key_id
        }
        KagemushaHardwarePlatformClassV1::AndroidKeyMint
            if pins.selected_attested_key_id == [0; 32] =>
        {
            [0; 32]
        }
        _ => return Err(SignedAppPreparationErrorV1::Binding),
    };
    let issued_at_ms = u64::from_le_bytes(
        token[1..9]
            .try_into()
            .map_err(|_| SignedAppPreparationErrorV1::Encoding)?,
    );
    let expires_at_ms = u64::from_le_bytes(
        token[9..17]
            .try_into()
            .map_err(|_| SignedAppPreparationErrorV1::Encoding)?,
    );
    let field = |start: usize| -> [u8; 32] {
        // Exact token length was checked above; each range is statically in bounds.
        token[start..start + 32]
            .try_into()
            .expect("fixed 32-byte field")
    };
    let preparation = VerifiedSignedAppPreparationV1 {
        issued_at_ms,
        expires_at_ms,
        client_nonce: field(17),
        server_nonce: field(49),
        release_id: field(81),
        profile_id: field(113),
        attested_key_id: field(145),
        lane_id: field(177),
    };
    if preparation.client_nonce != pins.client_nonce
        || preparation.server_nonce == [0; 32]
        || preparation.server_nonce == pins.client_nonce
        || preparation.release_id != pins.release_id
        || preparation.profile_id != pins.profile_id
        || preparation.attested_key_id != expected_key_id
        || preparation.lane_id != pins.lane_id
        || issued_at_ms == 0
        || issued_at_ms < pins.policy.valid_from_ms
        || expires_at_ms > pins.policy.expires_at_ms
        || issued_at_ms.checked_add(CHALLENGE_TTL_MS) != Some(expires_at_ms)
        || pins.trusted_now_ms < issued_at_ms
        || pins.trusted_now_ms >= expires_at_ms
    {
        return Err(SignedAppPreparationErrorV1::Binding);
    }

    let account = pins
        .account_id
        .canonical_i105()
        .map_err(|_| SignedAppPreparationErrorV1::Encoding)?;
    let mut message = Vec::with_capacity(DOMAIN.len() + 208 + 32 + 32);
    message.extend_from_slice(DOMAIN);
    message.extend_from_slice(&token[1..UNSIGNED_BYTES]);
    message.extend_from_slice(&pins.policy.issuer_policy_id);
    message.extend_from_slice(&Sha256::digest(account.as_bytes()));
    Signature::try_from_bytes(&token[UNSIGNED_BYTES..])
        .map_err(|_| SignedAppPreparationErrorV1::Authority)?
        .verify(&pins.policy.issuer_public_key, &message)
        .map_err(|_| SignedAppPreparationErrorV1::Authority)?;
    Ok(preparation)
}
