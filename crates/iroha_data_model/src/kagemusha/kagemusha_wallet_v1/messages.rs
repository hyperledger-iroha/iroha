//! Peer messages of the offline exchange (§§5.1, 8; design §4 with C2, C5 and C8).
//!
//! Offer, Request, session controls and policy data carry no monetary authority. A Payment
//! carries the receiver-signed Request, the payer credential, the complete Send package and the
//! signer certificates its verifier needs beyond the preinstalled scheme root; every component
//! is bound and canonical decoding is unique, so its structural digest binds every byte of the
//! canonical Payment. Credited is optional delivery evidence matched at the payer by receiver
//! wallet identity. Every message travels in one canonical envelope whose complete frame is
//! bounded per kind, and whose `kgm1:` text form is strict unpadded base64url.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1, KAGEMUSHA_WALLET_MESSAGE_TEXT_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1, KAGEMUSHA_WALLET_TEXT_PREFIX_V1,
    KAGEMUSHA_WALLET_VERSION_V1, KagemushaWalletValidationErrorV1, WalletResult, WalletVersionsV1,
    decode_frame_v1,
    digest::{
        KagemushaWalletDigestRoleV1 as Role, KagemushaWalletSignerOutputV1, WalletTranscriptV1,
        kagemusha_wallet_digest_v1, kagemusha_wallet_freeze_signature_v1,
        kagemusha_wallet_preimage_v1, kagemusha_wallet_signed_object_digest_v1,
        kagemusha_wallet_verify_signature_v1,
    },
    encode_frame_v1,
    identity::{
        KagemushaWalletCertificateSetV1, KagemushaWalletCredentialV1, KagemushaWalletSchemeV1,
        KagemushaWalletSignerCertificateV1, KagemushaWalletSignerRoleV1,
    },
    invalid_v1, is_zero_v1, overflow_v1,
    policy::{
        KagemushaWalletFeeScheduleV1, KagemushaWalletSchemePolicyV1, KagemushaWalletTimeIntervalV1,
    },
    require_nonzero_v1, require_scheme_v1, require_version_v1,
    state::{
        KagemushaWalletConsumedCreditLeafV1, KagemushaWalletEffectV1,
        KagemushaWalletFeeClaimLeafV1, KagemushaWalletPackageDigestsV1, KagemushaWalletPackageV1,
        KagemushaWalletPendingOutgoingLeafV1, KagemushaWalletProofV1,
        KagemushaWalletStateCommitmentV1, KagemushaWalletStateV1,
    },
};
use crate::kagemusha::KagemushaDeviceSignatureV1;

#[cfg(test)]
#[path = "messages_tests.rs"]
pub(super) mod messages_tests;

const DIGEST_BYTES: usize = 32;
const U16_BYTES: usize = 2;
const U64_BYTES: usize = 8;
const U128_BYTES: usize = 16;
const COMMITMENT_BYTES: usize = 2 * DIGEST_BYTES;

/// Exact `offer-body` transcript bytes.
pub const KAGEMUSHA_WALLET_OFFER_BODY_TRANSCRIPT_BYTES_V1: usize =
    U16_BYTES + 5 * DIGEST_BYTES + 2 * U128_BYTES;
/// Exact `request-body` transcript bytes; also the `credit` transcript.
pub const KAGEMUSHA_WALLET_REQUEST_BODY_TRANSCRIPT_BYTES_V1: usize =
    U16_BYTES + 9 * DIGEST_BYTES + 3 * U128_BYTES + 2 * U64_BYTES;
/// Exact positional Send `dependencies` transcript bytes: `LE32 3 || three digests`.
pub const KAGEMUSHA_WALLET_SEND_DEPENDENCIES_TRANSCRIPT_BYTES_V1: usize = 4 + 3 * DIGEST_BYTES;
/// Number of positional Send dependencies: payer issuer, receiver issuer and fee signer.
pub const KAGEMUSHA_WALLET_SEND_DEPENDENCIES_COUNT_V1: u32 = 3;
/// Exact `payment` transcript bytes.
pub const KAGEMUSHA_WALLET_PAYMENT_TRANSCRIPT_BYTES_V1: usize = U16_BYTES + 4 * DIGEST_BYTES;
/// Exact `credit-status-statement` transcript bytes.
pub const KAGEMUSHA_WALLET_CREDIT_STATUS_STATEMENT_TRANSCRIPT_BYTES_V1: usize =
    U16_BYTES + 7 * DIGEST_BYTES + COMMITMENT_BYTES + U128_BYTES + 2 * DIGEST_BYTES;
/// Exact `credited` transcript bytes.
pub const KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1: usize =
    U16_BYTES + 3 * DIGEST_BYTES + 1 + 4 * DIGEST_BYTES;

const SETUP_DECLINED_FIELDS_BYTES: usize = U16_BYTES;
const UNSUPPORTED_SCHEME_FIELDS_BYTES: usize = 0;
const RECEIVE_DEFERRED_FIELDS_BYTES: usize = U16_BYTES + DIGEST_BYTES;
const CLOSE_FIELDS_BYTES: usize = 0;
/// Kind-dependent field widths of every session control kind, in tag order.
const SESSION_CONTROL_FIELDS_BYTES: [usize; 4] = [
    SETUP_DECLINED_FIELDS_BYTES,
    UNSUPPORTED_SCHEME_FIELDS_BYTES,
    RECEIVE_DEFERRED_FIELDS_BYTES,
    CLOSE_FIELDS_BYTES,
];

const fn max_width_v1(widths: &[usize]) -> usize {
    let mut max = 0;
    let mut index = 0;
    while index < widths.len() {
        if widths[index] > max {
            max = widths[index];
        }
        index += 1;
    }
    max
}

/// Width of the kind-dependent session-control fields `LE16 reason || credit_id`: the largest
/// kind (`ReceiveDeferred`). Unused fields are zero.
pub const KAGEMUSHA_WALLET_SESSION_CONTROL_UNION_BYTES_V1: usize =
    max_width_v1(&SESSION_CONTROL_FIELDS_BYTES);
/// Exact `session-control-body` transcript bytes.
pub const KAGEMUSHA_WALLET_SESSION_CONTROL_BODY_TRANSCRIPT_BYTES_V1: usize =
    U16_BYTES + 5 * DIGEST_BYTES + 1 + KAGEMUSHA_WALLET_SESSION_CONTROL_UNION_BYTES_V1;

// ---------------------------------------------------------------------------------------
// Shared certificate helpers
// ---------------------------------------------------------------------------------------

/// Require that `certificates` holds exactly the certificates named by `required` — complete
/// and minimal — each with its role and under `scheme_id` (design §4.3).
pub(super) fn require_exact_certificates_v1(
    certificates: &KagemushaWalletCertificateSetV1,
    scheme_id: &[u8; 32],
    required: &[([u8; 32], KagemushaWalletSignerRoleV1)],
) -> WalletResult<()> {
    certificates.validate()?;
    let mut expected: Vec<[u8; 32]> = required.iter().map(|(digest, _)| *digest).collect();
    expected.sort_unstable();
    expected.dedup();
    if certificates.digests() != expected {
        return Err(invalid_v1("certificates.set"));
    }
    for (digest, role) in required {
        let certificate = certificates.certificate(digest, *role)?;
        require_scheme_v1(
            "certificate.scheme_id",
            &certificate.body.scheme_id,
            scheme_id,
        )?;
    }
    Ok(())
}

/// Select the certificate with `digest` and `role` from the first set that holds it.
pub(super) fn find_certificate_v1<'a>(
    sets: &[&'a KagemushaWalletCertificateSetV1],
    digest: &[u8; 32],
    role: KagemushaWalletSignerRoleV1,
) -> WalletResult<&'a KagemushaWalletSignerCertificateV1> {
    sets.iter()
        .find_map(|set| {
            set.certificates
                .iter()
                .find(|certificate| certificate.certificate_digest() == *digest)
        })
        .ok_or_else(|| invalid_v1("certificates.missing"))
        .and_then(|certificate| {
            if certificate.body.role == role {
                Ok(certificate)
            } else {
                Err(invalid_v1("certificate.role"))
            }
        })
}

/// Verify a credential under `scheme` with its Enrollment-role issuer from `certificates`.
pub(super) fn verify_credential_with_set_v1(
    credential: &KagemushaWalletCredentialV1,
    scheme: &KagemushaWalletSchemeV1,
    certificates: &KagemushaWalletCertificateSetV1,
) -> WalletResult<()> {
    certificates.verify(scheme)?;
    let issuer = certificates.certificate(
        &credential.body.issuer_certificate,
        KagemushaWalletSignerRoleV1::Enrollment,
    )?;
    credential.verify(scheme, issuer)
}

// ---------------------------------------------------------------------------------------
// Offer (§5.1, design §4.1)
// ---------------------------------------------------------------------------------------

/// Body of a payer Offer, signed by the payer payment key under `offer-body`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletOfferBodyV1"
)]
pub struct KagemushaWalletOfferBodyV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Offered scheme.
    pub scheme_id: [u8; 32],
    /// Offered asset scope digest.
    pub asset_digest: [u8; 32],
    /// Paying wallet.
    pub payer_wallet_id: [u8; 32],
    /// Digest of the payer credential carried beside the body.
    pub payer_credential_digest: [u8; 32],
    /// Payer's next send ordinal `s`.
    pub next_send: u128,
    /// Proposed positive amount.
    pub amount: u128,
    /// Fresh session nonce.
    pub session_nonce: [u8; 32],
}

impl KagemushaWalletOfferBodyV1 {
    /// Exact `offer-body` transcript.
    #[must_use]
    pub fn transcript(&self) -> Vec<u8> {
        WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_OFFER_BODY_TRANSCRIPT_BYTES_V1)
            .u16(self.version)
            .digest(&self.scheme_id)
            .digest(&self.asset_digest)
            .digest(&self.payer_wallet_id)
            .digest(&self.payer_credential_digest)
            .u128(self.next_send)
            .u128(self.amount)
            .digest(&self.session_nonce)
            .finish()
    }

    /// Signed body digest `H("offer-body", transcript)`.
    #[must_use]
    pub fn body_digest(&self) -> [u8; 32] {
        kagemusha_wallet_digest_v1(Role::OfferBody, &self.transcript())
    }

    /// Exact ECDSA message the payer payment key signs.
    #[must_use]
    pub fn signing_message(&self) -> Vec<u8> {
        kagemusha_wallet_preimage_v1(Role::OfferBody, &self.transcript())
    }

    /// Validate the body's fields.
    ///
    /// # Errors
    ///
    /// Rejects another version, zero bindings, and a zero amount.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("offer.version", self.version)?;
        for (field, digest) in [
            ("offer.scheme_id", &self.scheme_id),
            ("offer.asset_digest", &self.asset_digest),
            ("offer.payer_wallet_id", &self.payer_wallet_id),
            (
                "offer.payer_credential_digest",
                &self.payer_credential_digest,
            ),
            ("offer.session_nonce", &self.session_nonce),
        ] {
            require_nonzero_v1(field, digest)?;
        }
        if self.amount == 0 {
            return Err(invalid_v1("offer.amount"));
        }
        Ok(())
    }
}

/// Authenticated session hint of a payer: next ordinal, amount and scheme (§5.1).
///
/// It has no debit or credit authority. `certificates` is exactly the payer issuer certificate.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletOfferV1")]
pub struct KagemushaWalletOfferV1 {
    /// Signed body.
    pub body: KagemushaWalletOfferBodyV1,
    /// Payer credential.
    pub payer_credential: KagemushaWalletCredentialV1,
    /// Exactly the payer credential's issuer certificate.
    pub certificates: KagemushaWalletCertificateSetV1,
    /// Payer payment-key signature over `offer-body`.
    pub signature: KagemushaDeviceSignatureV1,
}

fn validate_offer_parts_v1(
    body: &KagemushaWalletOfferBodyV1,
    credential: &KagemushaWalletCredentialV1,
    certificates: &KagemushaWalletCertificateSetV1,
) -> WalletResult<()> {
    body.validate()?;
    credential.validate()?;
    let payer = &credential.body;
    require_scheme_v1("offer.scheme_id", &body.scheme_id, &payer.scheme_id)?;
    if body.asset_digest != payer.asset_digest {
        return Err(invalid_v1("offer.asset_digest"));
    }
    if body.payer_wallet_id != payer.wallet_id {
        return Err(invalid_v1("offer.payer_wallet_id"));
    }
    if body.payer_credential_digest != credential.credential_digest() {
        return Err(invalid_v1("offer.payer_credential_digest"));
    }
    require_exact_certificates_v1(
        certificates,
        &body.scheme_id,
        &[(
            payer.issuer_certificate,
            KagemushaWalletSignerRoleV1::Enrollment,
        )],
    )
}

impl KagemushaWalletOfferV1 {
    /// Freeze the payer payment-key signature over `body`.
    ///
    /// # Errors
    ///
    /// Rejects an invalid body or credential, a body for another wallet or credential, an
    /// issuer certificate that is not the credential's, or a signature that does not verify.
    pub fn sign(
        body: KagemushaWalletOfferBodyV1,
        payer_credential: KagemushaWalletCredentialV1,
        issuer_certificate: &KagemushaWalletSignerCertificateV1,
        signer_output: KagemushaWalletSignerOutputV1<'_>,
    ) -> WalletResult<Self> {
        let certificates = KagemushaWalletCertificateSetV1::new(vec![*issuer_certificate])?;
        validate_offer_parts_v1(&body, &payer_credential, &certificates)?;
        let signature = kagemusha_wallet_freeze_signature_v1(
            &payer_credential.body.payment_key,
            Role::OfferBody,
            &body.transcript(),
            signer_output,
        )?;
        Ok(Self {
            body,
            payer_credential,
            certificates,
            signature,
        })
    }

    /// Validate the Offer and verify its payer signature.
    ///
    /// # Errors
    ///
    /// Rejects an invalid body or credential, a body that does not name the credential's
    /// wallet, scheme, asset or digest, a certificate set other than exactly the issuer
    /// certificate, and a signature that does not verify under the payer payment key.
    pub fn validate(&self) -> WalletResult<()> {
        validate_offer_parts_v1(&self.body, &self.payer_credential, &self.certificates)?;
        kagemusha_wallet_verify_signature_v1(
            &self.payer_credential.body.payment_key,
            Role::OfferBody,
            &self.body.transcript(),
            &self.signature,
        )
    }

    /// Validate the Offer and verify its credential and certificate under `scheme`.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::validate`] rejects, another scheme, and issuer or root signatures
    /// that do not verify.
    pub fn verify(&self, scheme: &KagemushaWalletSchemeV1) -> WalletResult<()> {
        self.validate()?;
        verify_credential_with_set_v1(&self.payer_credential, scheme, &self.certificates)
    }
}

// ---------------------------------------------------------------------------------------
// Request (§§5.1, 6.2, design §4.2 and C5)
// ---------------------------------------------------------------------------------------

/// Signed fee schedule named by a Request, or none for a zero fee.
// The schedule is a fixed-size signed value; boxing it would only add an allocation to a
// short-lived quote while the wire shape stays the same.
#[allow(
    clippy::large_enum_variant,
    reason = "the fixed-size signed schedule stays inline in the canonical wire value"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletFeeScheduleSlotV1"
)]
pub enum KagemushaWalletFeeScheduleSlotV1 {
    /// No fee schedule; the fee is zero.
    #[codec(index = 0)]
    None,
    /// The signed fee schedule whose digest the Request body names.
    #[codec(index = 1)]
    Present {
        /// Signed immutable fee schedule.
        schedule: KagemushaWalletFeeScheduleV1,
    },
}

impl KagemushaWalletFeeScheduleSlotV1 {
    /// Wire tag.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Present { .. } => 1,
        }
    }

    /// The carried schedule, if any.
    #[must_use]
    pub const fn schedule(&self) -> Option<&KagemushaWalletFeeScheduleV1> {
        match self {
            Self::None => None,
            Self::Present { schedule } => Some(schedule),
        }
    }

    /// Digest of the carried schedule, or zero.
    #[must_use]
    pub fn fee_schedule_digest(&self) -> [u8; 32] {
        self.schedule()
            .map_or([0; 32], KagemushaWalletFeeScheduleV1::fee_schedule_digest)
    }

    /// Certificate digest of the carried schedule's signer, or zero.
    #[must_use]
    pub fn signer_certificate(&self) -> [u8; 32] {
        self.schedule()
            .map_or([0; 32], |schedule| schedule.body.signer_certificate)
    }
}

/// Body of a receiver Request, signed by the receiver payment key under `request-body`.
///
/// Its transcript also defines `credit_id = H("credit", transcript)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletRequestBodyV1"
)]
pub struct KagemushaWalletRequestBodyV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Scheme.
    pub scheme_id: [u8; 32],
    /// Asset scope digest.
    pub asset_digest: [u8; 32],
    /// Paying wallet.
    pub payer_wallet_id: [u8; 32],
    /// Receiving wallet; the receiver credential's wallet.
    pub receiver_wallet_id: [u8; 32],
    /// Payer send ordinal `s` this quote is scoped to.
    pub send_ordinal: u128,
    /// Digest of the receiver credential carried beside the body.
    pub receiver_credential_digest: [u8; 32],
    /// Exact positive amount credited to the receiver.
    pub amount: u128,
    /// Fee schedule digest; zero for no fee.
    pub fee_schedule: [u8; 32],
    /// Exact fee under the schedule; zero without one.
    pub fee: u128,
    /// Receiver's scheme policy epoch; zero when none is held.
    pub policy_epoch: u64,
    /// Receiver's scheme policy digest; zero when none is held.
    pub scheme_policy: [u8; 32],
    /// Receiver's authenticated accepted time in Unix milliseconds.
    pub receiver_accepted_time_ms: u64,
    /// Digest of the Request's certificate set.
    pub certificates: [u8; 32],
    /// Fresh 256-bit nonce.
    pub nonce: [u8; 32],
}

impl KagemushaWalletRequestBodyV1 {
    /// Exact `request-body` transcript.
    #[must_use]
    pub fn transcript(&self) -> Vec<u8> {
        WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_REQUEST_BODY_TRANSCRIPT_BYTES_V1)
            .u16(self.version)
            .digest(&self.scheme_id)
            .digest(&self.asset_digest)
            .digest(&self.payer_wallet_id)
            .digest(&self.receiver_wallet_id)
            .u128(self.send_ordinal)
            .digest(&self.receiver_credential_digest)
            .u128(self.amount)
            .digest(&self.fee_schedule)
            .u128(self.fee)
            .u64(self.policy_epoch)
            .digest(&self.scheme_policy)
            .u64(self.receiver_accepted_time_ms)
            .digest(&self.certificates)
            .digest(&self.nonce)
            .finish()
    }

    /// Signed body digest `e = H("request-body", transcript)`.
    #[must_use]
    pub fn body_digest(&self) -> [u8; 32] {
        kagemusha_wallet_digest_v1(Role::RequestBody, &self.transcript())
    }

    /// Exact ECDSA message the receiver payment key signs.
    #[must_use]
    pub fn signing_message(&self) -> Vec<u8> {
        kagemusha_wallet_preimage_v1(Role::RequestBody, &self.transcript())
    }

    /// Credit identity `H("credit", request-body transcript)` (§5.1).
    #[must_use]
    pub fn credit_id(&self) -> [u8; 32] {
        kagemusha_wallet_digest_v1(Role::Credit, &self.transcript())
    }

    /// Validate the body's self-contained rules.
    ///
    /// # Errors
    ///
    /// Rejects another version, zero bindings, a zero amount, a payer equal to the receiver,
    /// an overflowing gross debit, a policy epoch that disagrees with its scheme-policy digest,
    /// and a nonzero fee without a fee schedule.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("request.version", self.version)?;
        for (field, digest) in [
            ("request.scheme_id", &self.scheme_id),
            ("request.asset_digest", &self.asset_digest),
            ("request.payer_wallet_id", &self.payer_wallet_id),
            ("request.receiver_wallet_id", &self.receiver_wallet_id),
            (
                "request.receiver_credential_digest",
                &self.receiver_credential_digest,
            ),
            ("request.certificates", &self.certificates),
            ("request.nonce", &self.nonce),
        ] {
            require_nonzero_v1(field, digest)?;
        }
        if self.amount == 0 {
            return Err(invalid_v1("request.amount"));
        }
        if self.payer_wallet_id == self.receiver_wallet_id {
            return Err(invalid_v1("request.payer_wallet_id"));
        }
        self.amount
            .checked_add(self.fee)
            .ok_or_else(|| overflow_v1("request.gross"))?;
        if (self.policy_epoch == 0) != is_zero_v1(&self.scheme_policy) {
            return Err(invalid_v1("request.scheme_policy"));
        }
        if is_zero_v1(&self.fee_schedule) && self.fee != 0 {
            return Err(invalid_v1("request.fee"));
        }
        Ok(())
    }
}

/// Receiver-signed nonmonetary setup quote (§5.1).
///
/// It creates no state transition, receiver ordinal or receive slot. `certificates` is exactly
/// the receiver issuer certificate and, with a fee schedule, the schedule's signer certificate.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletRequestV1"
)]
pub struct KagemushaWalletRequestV1 {
    /// Signed body.
    pub body: KagemushaWalletRequestBodyV1,
    /// Receiver credential.
    pub receiver_credential: KagemushaWalletCredentialV1,
    /// Fee schedule named by the body, if any.
    pub fee_schedule: KagemushaWalletFeeScheduleSlotV1,
    /// Receiver issuer certificate and fee-schedule signer certificate.
    pub certificates: KagemushaWalletCertificateSetV1,
    /// Receiver payment-key signature over `request-body`.
    pub signature: KagemushaDeviceSignatureV1,
}

fn validate_request_parts_v1(
    body: &KagemushaWalletRequestBodyV1,
    credential: &KagemushaWalletCredentialV1,
    fee_schedule: &KagemushaWalletFeeScheduleSlotV1,
    certificates: &KagemushaWalletCertificateSetV1,
) -> WalletResult<()> {
    body.validate()?;
    credential.validate()?;
    let receiver = &credential.body;
    require_scheme_v1("request.scheme_id", &body.scheme_id, &receiver.scheme_id)?;
    if body.asset_digest != receiver.asset_digest {
        return Err(invalid_v1("request.asset_digest"));
    }
    if body.receiver_wallet_id != receiver.wallet_id {
        return Err(invalid_v1("request.receiver_wallet_id"));
    }
    if body.receiver_credential_digest != credential.credential_digest() {
        return Err(invalid_v1("request.receiver_credential_digest"));
    }
    let mut required = vec![(
        receiver.issuer_certificate,
        KagemushaWalletSignerRoleV1::Enrollment,
    )];
    match fee_schedule {
        KagemushaWalletFeeScheduleSlotV1::None => {
            if !is_zero_v1(&body.fee_schedule) {
                return Err(invalid_v1("request.fee_schedule"));
            }
        }
        KagemushaWalletFeeScheduleSlotV1::Present { schedule } => {
            schedule.validate()?;
            require_scheme_v1(
                "fee_schedule.scheme_id",
                &schedule.body.scheme_id,
                &body.scheme_id,
            )?;
            if schedule.body.asset_digest != body.asset_digest {
                return Err(invalid_v1("fee_schedule.asset_digest"));
            }
            if schedule.fee_schedule_digest() != body.fee_schedule {
                return Err(invalid_v1("request.fee_schedule"));
            }
            if schedule.fee(body.amount)? != body.fee {
                return Err(invalid_v1("request.fee"));
            }
            required.push((
                schedule.body.signer_certificate,
                KagemushaWalletSignerRoleV1::RegulatoryPolicy,
            ));
        }
    }
    require_exact_certificates_v1(certificates, &body.scheme_id, &required)?;
    if certificates.digest()? != body.certificates {
        return Err(invalid_v1("request.certificates"));
    }
    Ok(())
}

impl KagemushaWalletRequestV1 {
    /// Freeze the receiver payment-key signature over `body`.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::validate`] rejects before the signature, and a signature that does
    /// not verify under the receiver payment key.
    pub fn sign(
        body: KagemushaWalletRequestBodyV1,
        receiver_credential: KagemushaWalletCredentialV1,
        fee_schedule: KagemushaWalletFeeScheduleSlotV1,
        certificates: KagemushaWalletCertificateSetV1,
        signer_output: KagemushaWalletSignerOutputV1<'_>,
    ) -> WalletResult<Self> {
        validate_request_parts_v1(&body, &receiver_credential, &fee_schedule, &certificates)?;
        let signature = kagemusha_wallet_freeze_signature_v1(
            &receiver_credential.body.payment_key,
            Role::RequestBody,
            &body.transcript(),
            signer_output,
        )?;
        Ok(Self {
            body,
            receiver_credential,
            fee_schedule,
            certificates,
            signature,
        })
    }

    /// Credit identity of this Request.
    #[must_use]
    pub fn credit_id(&self) -> [u8; 32] {
        self.body.credit_id()
    }

    /// Request digest `H("request", e || signature)`.
    #[must_use]
    pub fn request_digest(&self) -> [u8; 32] {
        kagemusha_wallet_signed_object_digest_v1(
            Role::Request,
            &self.body.body_digest(),
            &self.signature,
        )
    }

    /// Validate the Request and verify its receiver signature (§5.1, design §4.2 and C5).
    ///
    /// # Errors
    ///
    /// Rejects an invalid body or credential, a body that does not name the receiver
    /// credential's wallet, scheme, asset or digest, a fee-schedule slot that disagrees with
    /// the body or prices another fee, a schedule for another scheme or asset, a certificate
    /// set other than exactly the receiver issuer and fee signer, a set digest that differs
    /// from the body, and a signature that does not verify under the receiver payment key.
    pub fn validate(&self) -> WalletResult<()> {
        validate_request_parts_v1(
            &self.body,
            &self.receiver_credential,
            &self.fee_schedule,
            &self.certificates,
        )?;
        kagemusha_wallet_verify_signature_v1(
            &self.receiver_credential.body.payment_key,
            Role::RequestBody,
            &self.body.transcript(),
            &self.signature,
        )
    }

    /// Validate the Request and verify its credential, fee schedule and certificates under
    /// `scheme`.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::validate`] rejects, another scheme, and issuer, policy or root
    /// signatures that do not verify.
    pub fn verify(&self, scheme: &KagemushaWalletSchemeV1) -> WalletResult<()> {
        self.validate()?;
        verify_credential_with_set_v1(&self.receiver_credential, scheme, &self.certificates)?;
        if let Some(schedule) = self.fee_schedule.schedule() {
            let signer = self.certificates.certificate(
                &schedule.body.signer_certificate,
                KagemushaWalletSignerRoleV1::RegulatoryPolicy,
            )?;
            schedule.verify(scheme, signer)?;
        }
        Ok(())
    }

    /// Native Send-rule pre-check of the payer (design C5).
    ///
    /// The Send relation enforces the same rules; the controls of §7 are checked separately
    /// with the state's lease, blacklist and quota checks.
    ///
    /// # Errors
    ///
    /// Rejects an invalid Request or payer state, another scheme or asset, a Request for
    /// another payer or ordinal, a payment to the payer itself or to the same key, a payer
    /// policy epoch below the Request's, a different scheme policy at the same epoch, a
    /// different fee schedule, and a balance below `amount + fee`.
    pub fn check_send_rule(
        &self,
        payer_credential: &KagemushaWalletCredentialV1,
        payer_state: &KagemushaWalletStateV1,
    ) -> WalletResult<()> {
        self.validate()?;
        payer_state.validate_for_credential(payer_credential)?;
        let body = &self.body;
        require_scheme_v1("request.scheme_id", &body.scheme_id, &payer_state.scheme_id)?;
        if body.asset_digest != payer_state.asset_digest {
            return Err(invalid_v1("request.asset_digest"));
        }
        if body.payer_wallet_id != payer_state.wallet_id {
            return Err(invalid_v1("request.payer_wallet_id"));
        }
        if body.receiver_wallet_id == payer_state.wallet_id {
            return Err(invalid_v1("request.receiver_wallet_id"));
        }
        if payer_credential.body.payment_key == self.receiver_credential.body.payment_key {
            return Err(invalid_v1("request.payment_key"));
        }
        if body.send_ordinal != payer_state.next_send {
            return Err(invalid_v1("request.send_ordinal"));
        }
        let policy = &payer_state.policy;
        if policy.policy_epoch < body.policy_epoch {
            return Err(invalid_v1("request.policy_epoch"));
        }
        if policy.policy_epoch == body.policy_epoch && policy.scheme_policy != body.scheme_policy {
            return Err(invalid_v1("request.scheme_policy"));
        }
        if body.fee_schedule != policy.fee_schedule {
            return Err(invalid_v1("request.fee_schedule"));
        }
        let gross = body
            .amount
            .checked_add(body.fee)
            .ok_or_else(|| overflow_v1("request.gross"))?;
        if payer_state.balance < gross {
            return Err(invalid_v1("state.balance"));
        }
        Ok(())
    }

    /// Require that `payer_credential` is the payer this Request names: same scheme and asset,
    /// the Request's payer wallet, and a payment key other than the receiver's (design §4.3,
    /// C5).
    fn require_payer(&self, payer_credential: &KagemushaWalletCredentialV1) -> WalletResult<()> {
        let body = &self.body;
        let payer = &payer_credential.body;
        require_scheme_v1(
            "request.payer_credential.scheme_id",
            &payer.scheme_id,
            &body.scheme_id,
        )?;
        if payer.asset_digest != body.asset_digest {
            return Err(invalid_v1("request.payer_credential.asset_digest"));
        }
        if payer.wallet_id != body.payer_wallet_id {
            return Err(invalid_v1("request.payer_wallet_id"));
        }
        if payer.payment_key == self.receiver_credential.body.payment_key {
            return Err(invalid_v1("request.payment_key"));
        }
        Ok(())
    }

    /// Positional verification dependencies of a Send by `payer_credential` under this Request
    /// (§4.3).
    ///
    /// # Errors
    ///
    /// Rejects a credential that is not the payer this Request names.
    pub fn send_dependencies(
        &self,
        payer_credential: &KagemushaWalletCredentialV1,
    ) -> WalletResult<[u8; 32]> {
        self.require_payer(payer_credential)?;
        Ok(kagemusha_wallet_send_dependencies_v1(
            &payer_credential.body.issuer_certificate,
            &self.receiver_credential.body.issuer_certificate,
            &self.fee_schedule.signer_certificate(),
        ))
    }

    /// Send effect of this Request at the effective accepted time `interval` (§7).
    ///
    /// The Send is irreversible once committed, so the payer credential must be the payer the
    /// Request names before any effect binds its issuer certificate.
    ///
    /// # Errors
    ///
    /// Rejects an invalid Request or payer credential, a credential for another scheme, asset
    /// or payer wallet, a payer key equal to the receiver's, an inverted interval, and a lower
    /// bound below the receiver's accepted time.
    pub fn send_effect(
        &self,
        payer_credential: &KagemushaWalletCredentialV1,
        interval: &KagemushaWalletTimeIntervalV1,
    ) -> WalletResult<KagemushaWalletEffectV1> {
        self.validate()?;
        payer_credential.validate()?;
        self.require_payer(payer_credential)?;
        if interval.lower_ms > interval.upper_ms
            || interval.lower_ms < self.body.receiver_accepted_time_ms
        {
            return Err(invalid_v1("effect.accepted_time"));
        }
        let effect = KagemushaWalletEffectV1::Send {
            credit_id: self.credit_id(),
            receiver_wallet_id: self.body.receiver_wallet_id,
            send_ordinal: self.body.send_ordinal,
            amount: self.body.amount,
            fee: self.body.fee,
            request: self.request_digest(),
            dependencies: self.send_dependencies(payer_credential)?,
            accepted_lower_ms: interval.lower_ms,
            accepted_upper_ms: interval.upper_ms,
        };
        effect.validate()?;
        Ok(effect)
    }
}

/// Exact positional Send `dependencies` transcript:
/// `LE32 3 || payer issuer || receiver issuer || fee signer or zero`.
#[must_use]
pub fn kagemusha_wallet_send_dependencies_transcript_v1(
    payer_issuer_certificate: &[u8; 32],
    receiver_issuer_certificate: &[u8; 32],
    fee_signer_certificate: &[u8; 32],
) -> Vec<u8> {
    WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_SEND_DEPENDENCIES_TRANSCRIPT_BYTES_V1)
        .u32(KAGEMUSHA_WALLET_SEND_DEPENDENCIES_COUNT_V1)
        .digest(payer_issuer_certificate)
        .digest(receiver_issuer_certificate)
        .digest(fee_signer_certificate)
        .finish()
}

/// Positional Send dependencies digest `H("dependencies", transcript)` bound by the Send effect.
#[must_use]
pub fn kagemusha_wallet_send_dependencies_v1(
    payer_issuer_certificate: &[u8; 32],
    receiver_issuer_certificate: &[u8; 32],
    fee_signer_certificate: &[u8; 32],
) -> [u8; 32] {
    kagemusha_wallet_digest_v1(
        Role::Dependencies,
        &kagemusha_wallet_send_dependencies_transcript_v1(
            payer_issuer_certificate,
            receiver_issuer_certificate,
            fee_signer_certificate,
        ),
    )
}

// ---------------------------------------------------------------------------------------
// Payment (§§5.1, 8, design §4.3 and C5)
// ---------------------------------------------------------------------------------------

/// Exact `payment` transcript:
/// `LE16 version || request_digest || payer_credential_digest || package_digest || set digest`.
#[must_use]
pub fn kagemusha_wallet_payment_transcript_v1(
    request_digest: &[u8; 32],
    payer_credential_digest: &[u8; 32],
    package_digest: &[u8; 32],
    certificates_digest: &[u8; 32],
) -> Vec<u8> {
    WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_PAYMENT_TRANSCRIPT_BYTES_V1)
        .u16(KAGEMUSHA_WALLET_VERSION_V1)
        .digest(request_digest)
        .digest(payer_credential_digest)
        .digest(package_digest)
        .digest(certificates_digest)
        .finish()
}

/// Digests of one fully validated Payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KagemushaWalletPaymentDigestsV1 {
    /// Credit identity of the Request.
    pub credit_id: [u8; 32],
    /// Request digest.
    pub request: [u8; 32],
    /// Payer credential digest.
    pub payer_credential: [u8; 32],
    /// Digests of the Send package.
    pub package: KagemushaWalletPackageDigestsV1,
    /// Digest of the Payment's certificate set.
    pub certificates: [u8; 32],
    /// Payment digest `H("payment", transcript)`.
    pub payment: [u8; 32],
}

/// Complete canonical Payment: Request, payer credential, Send package and the certificates
/// the verifier needs that the Request does not carry (§§5.1, 8).
///
/// The Payment is committed and retained before first release; delivery retries present the
/// exact same bytes.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletPaymentV1"
)]
pub struct KagemushaWalletPaymentV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Receiver-signed Request.
    pub request: KagemushaWalletRequestV1,
    /// Payer credential.
    pub payer_credential: KagemushaWalletCredentialV1,
    /// Complete committed Send package.
    pub send: KagemushaWalletPackageV1,
    /// Exactly the needed certificates the Request does not carry.
    pub certificates: KagemushaWalletCertificateSetV1,
}

impl KagemushaWalletPaymentV1 {
    /// Assemble the canonical Payment of a committed Send.
    ///
    /// The certificate set is the payer issuer certificate unless the Request already carries
    /// it.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::digests`] rejects.
    pub fn assemble(
        request: KagemushaWalletRequestV1,
        payer_credential: KagemushaWalletCredentialV1,
        payer_issuer_certificate: &KagemushaWalletSignerCertificateV1,
        send: KagemushaWalletPackageV1,
    ) -> WalletResult<Self> {
        let carried = request
            .certificates
            .digests()
            .contains(&payer_issuer_certificate.certificate_digest());
        let certificates = if carried {
            KagemushaWalletCertificateSetV1::default()
        } else {
            KagemushaWalletCertificateSetV1::new(vec![*payer_issuer_certificate])?
        };
        let payment = Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            request,
            payer_credential,
            send,
            certificates,
        };
        payment.digests()?;
        Ok(payment)
    }

    /// Fully validate the Payment and return its digests (design §4.3 and C5).
    ///
    /// No Payment digest is exposed without this validation. Cheap bindings are checked before
    /// the receipt and Request signatures.
    ///
    /// # Errors
    ///
    /// Rejects another version; an invalid Request or payer credential; a payer credential for
    /// another scheme, asset or wallet; equal payer and receiver keys; a certificate set that
    /// is not exactly the needed certificates the Request lacks; a package that is not a Send
    /// or whose credit, receiver, ordinal, amount, fee, Request, dependencies or accepted time
    /// differ from the Request; an invalid package or receipt; and signatures that do not
    /// verify.
    pub fn digests(&self) -> WalletResult<KagemushaWalletPaymentDigestsV1> {
        require_version_v1("payment.version", self.version)?;
        let request = &self.request;
        let body = &request.body;
        validate_request_parts_v1(
            body,
            &request.receiver_credential,
            &request.fee_schedule,
            &request.certificates,
        )?;
        let payer = &self.payer_credential;
        payer.validate()?;
        require_scheme_v1(
            "payment.payer_credential.scheme_id",
            &payer.body.scheme_id,
            &body.scheme_id,
        )?;
        if payer.body.asset_digest != body.asset_digest {
            return Err(invalid_v1("payment.payer_credential.asset_digest"));
        }
        if payer.body.wallet_id != body.payer_wallet_id {
            return Err(invalid_v1("payment.payer_wallet_id"));
        }
        if payer.body.payment_key == request.receiver_credential.body.payment_key {
            return Err(invalid_v1("payment.payment_key"));
        }
        let payer_issuer = payer.body.issuer_certificate;
        if request.certificates.digests().contains(&payer_issuer) {
            request
                .certificates
                .certificate(&payer_issuer, KagemushaWalletSignerRoleV1::Enrollment)?;
            require_exact_certificates_v1(&self.certificates, &body.scheme_id, &[])?;
        } else {
            require_exact_certificates_v1(
                &self.certificates,
                &body.scheme_id,
                &[(payer_issuer, KagemushaWalletSignerRoleV1::Enrollment)],
            )?;
        }
        let credit_id = request.credit_id();
        let request_digest = request.request_digest();
        let dependencies = request.send_dependencies(payer)?;
        match self.send.statement.effect {
            KagemushaWalletEffectV1::Send {
                credit_id: effect_credit_id,
                receiver_wallet_id,
                send_ordinal,
                amount,
                fee,
                request: effect_request,
                dependencies: effect_dependencies,
                accepted_lower_ms,
                ..
            } => {
                for (field, matches) in [
                    ("payment.effect.credit_id", effect_credit_id == credit_id),
                    (
                        "payment.effect.receiver_wallet_id",
                        receiver_wallet_id == body.receiver_wallet_id,
                    ),
                    (
                        "payment.effect.send_ordinal",
                        send_ordinal == body.send_ordinal,
                    ),
                    ("payment.effect.amount", amount == body.amount),
                    ("payment.effect.fee", fee == body.fee),
                    ("payment.effect.request", effect_request == request_digest),
                    (
                        "payment.effect.dependencies",
                        effect_dependencies == dependencies,
                    ),
                    (
                        "payment.effect.accepted_time",
                        accepted_lower_ms >= body.receiver_accepted_time_ms,
                    ),
                ] {
                    if !matches {
                        return Err(invalid_v1(field));
                    }
                }
            }
            _ => return Err(invalid_v1("payment.effect")),
        }
        let package = self.send.verify(payer)?;
        kagemusha_wallet_verify_signature_v1(
            &request.receiver_credential.body.payment_key,
            Role::RequestBody,
            &body.transcript(),
            &request.signature,
        )?;
        let certificates = self.certificates.digest()?;
        let payer_credential = payer.credential_digest();
        let payment = kagemusha_wallet_digest_v1(
            Role::Payment,
            &kagemusha_wallet_payment_transcript_v1(
                &request_digest,
                &payer_credential,
                &package.package,
                &certificates,
            ),
        );
        Ok(KagemushaWalletPaymentDigestsV1 {
            credit_id,
            request: request_digest,
            payer_credential,
            package,
            certificates,
            payment,
        })
    }

    /// Fully validate the Payment.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::digests`] rejects.
    pub fn validate(&self) -> WalletResult<()> {
        self.digests().map(|_| ())
    }

    /// Payment digest of the validated Payment.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::digests`] rejects.
    pub fn payment_digest(&self) -> WalletResult<[u8; 32]> {
        Ok(self.digests()?.payment)
    }

    /// Validate the Payment and verify every credential, schedule, certificate and the
    /// statement's relation under `scheme`.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::digests`] rejects, another scheme or relation, and issuer, policy
    /// or root signatures that do not verify.
    pub fn verify(
        &self,
        scheme: &KagemushaWalletSchemeV1,
    ) -> WalletResult<KagemushaWalletPaymentDigestsV1> {
        let digests = self.digests()?;
        self.request.verify(scheme)?;
        self.certificates.verify(scheme)?;
        let issuer = find_certificate_v1(
            &[&self.request.certificates, &self.certificates],
            &self.payer_credential.body.issuer_certificate,
            KagemushaWalletSignerRoleV1::Enrollment,
        )?;
        self.payer_credential.verify(scheme, issuer)?;
        self.send.statement.validate_for_scheme(scheme)?;
        Ok(digests)
    }

    /// Receive effect of this Payment for the receiver holding `receiver_credential`.
    ///
    /// The receiver matches on `request.body.receiver_wallet_id`, never on credential-digest
    /// equality, so a renewed receiver credential still receives (design C5). Consumed-credit
    /// nonmembership is proved by the Receive relation.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::digests`] rejects, an invalid receiver credential, and a Payment
    /// for another scheme, asset or wallet.
    pub fn receive_effect(
        &self,
        receiver_credential: &KagemushaWalletCredentialV1,
    ) -> WalletResult<KagemushaWalletEffectV1> {
        let digests = self.digests()?;
        receiver_credential.validate()?;
        let body = &self.request.body;
        let receiver = &receiver_credential.body;
        require_scheme_v1("payment.scheme_id", &body.scheme_id, &receiver.scheme_id)?;
        if body.asset_digest != receiver.asset_digest {
            return Err(invalid_v1("payment.asset_digest"));
        }
        if body.receiver_wallet_id != receiver.wallet_id {
            return Err(invalid_v1("payment.receiver_wallet_id"));
        }
        Ok(KagemushaWalletEffectV1::Receive {
            credit_id: digests.credit_id,
            payer_wallet_id: body.payer_wallet_id,
            payment: digests.payment,
            amount: body.amount,
        })
    }

    /// Pending-outgoing leaf the payer's Send inserts (§3.2).
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::digests`] rejects.
    pub fn pending_outgoing_leaf(&self) -> WalletResult<KagemushaWalletPendingOutgoingLeafV1> {
        let digests = self.digests()?;
        let body = &self.request.body;
        Ok(KagemushaWalletPendingOutgoingLeafV1 {
            credit_id: digests.credit_id,
            receiver_wallet_id: body.receiver_wallet_id,
            send_ordinal: body.send_ordinal,
            amount: body.amount,
            fee: body.fee,
            request_digest: digests.request,
        })
    }

    /// Consumed-credit leaf the receiver's Receive inserts permanently (§3.2).
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::digests`] rejects.
    pub fn consumed_credit_leaf(&self) -> WalletResult<KagemushaWalletConsumedCreditLeafV1> {
        let digests = self.digests()?;
        Ok(KagemushaWalletConsumedCreditLeafV1 {
            credit_id: digests.credit_id,
            payment_digest: digests.payment,
        })
    }

    /// Fee-claim leaf of a nonzero fee, or `None` for a zero fee (§6.2).
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::digests`] rejects.
    pub fn fee_claim_leaf(&self) -> WalletResult<Option<KagemushaWalletFeeClaimLeafV1>> {
        let digests = self.digests()?;
        let body = &self.request.body;
        if body.fee == 0 {
            return Ok(None);
        }
        Ok(Some(KagemushaWalletFeeClaimLeafV1 {
            credit_id: digests.credit_id,
            fee: body.fee,
            fee_schedule_digest: body.fee_schedule,
        }))
    }

    /// Validate and encode the bounded canonical frame of the released output bytes.
    ///
    /// # Errors
    ///
    /// Rejects an invalid Payment or a frame above the message bound.
    pub fn to_canonical_bytes(&self) -> WalletResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)
    }

    /// Decode one canonical Payment frame for `expected_scheme_id` and fully validate it.
    ///
    /// # Errors
    ///
    /// Rejects, in order, an oversized frame, a noncanonical frame, another version, another
    /// scheme, and what [`Self::digests`] rejects.
    pub fn decode_canonical(bytes: &[u8], expected_scheme_id: &[u8; 32]) -> WalletResult<Self> {
        let payment: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?;
        payment.require_versions()?;
        require_scheme_v1(
            "payment.scheme_id",
            &payment.request.body.scheme_id,
            expected_scheme_id,
        )?;
        payment.validate()?;
        Ok(payment)
    }
}

// ---------------------------------------------------------------------------------------
// CreditStatus and Credited (§5.1, design §4.4 and C5)
// ---------------------------------------------------------------------------------------

/// Public statement of a read-only `CreditStatus` proof: consumed-credit membership of
/// `(credit_id, payment_digest)` in the receiver's current complete package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletCreditStatusStatementV1"
)]
pub struct KagemushaWalletCreditStatusStatementV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Scheme.
    pub scheme_id: [u8; 32],
    /// Relation identity.
    pub relation_id: [u8; 32],
    /// Asset scope digest.
    pub asset_digest: [u8; 32],
    /// Receiving wallet.
    pub receiver_wallet_id: [u8; 32],
    /// Digest of the receiver's current credential.
    pub receiver_credential_digest: [u8; 32],
    /// Consumed credit identity.
    pub credit_id: [u8; 32],
    /// Digest of the full canonical Payment bound to that credit.
    pub payment_digest: [u8; 32],
    /// Successor commitment of the current package.
    pub current: KagemushaWalletStateCommitmentV1,
    /// Sequence of the current package.
    pub current_sequence: u128,
    /// Statement digest of the current package.
    pub current_statement_digest: [u8; 32],
    /// Receipt digest of the current package.
    pub current_receipt_digest: [u8; 32],
}

impl KagemushaWalletCreditStatusStatementV1 {
    /// Statement of `(credit_id, payment_digest)` against the receiver's verified `current`
    /// package.
    ///
    /// # Errors
    ///
    /// Rejects a package that does not verify under `receiver_credential`.
    pub fn for_current(
        receiver_credential: &KagemushaWalletCredentialV1,
        current: &KagemushaWalletPackageV1,
        credit_id: [u8; 32],
        payment_digest: [u8; 32],
    ) -> WalletResult<Self> {
        let digests = current.verify(receiver_credential)?;
        let statement = &current.statement;
        let status = Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: statement.scheme_id,
            relation_id: statement.relation_id,
            asset_digest: statement.asset_digest,
            receiver_wallet_id: receiver_credential.body.wallet_id,
            receiver_credential_digest: receiver_credential.credential_digest(),
            credit_id,
            payment_digest,
            current: statement.successor,
            current_sequence: statement.sequence,
            current_statement_digest: digests.statement,
            current_receipt_digest: digests.receipt,
        };
        status.validate()?;
        Ok(status)
    }

    /// Exact `credit-status-statement` transcript.
    #[must_use]
    pub fn transcript(&self) -> Vec<u8> {
        WalletTranscriptV1::with_capacity(
            KAGEMUSHA_WALLET_CREDIT_STATUS_STATEMENT_TRANSCRIPT_BYTES_V1,
        )
        .u16(self.version)
        .digest(&self.scheme_id)
        .digest(&self.relation_id)
        .digest(&self.asset_digest)
        .digest(&self.receiver_wallet_id)
        .digest(&self.receiver_credential_digest)
        .digest(&self.credit_id)
        .digest(&self.payment_digest)
        .digest(&self.current.eq)
        .digest(&self.current.ep)
        .u128(self.current_sequence)
        .digest(&self.current_statement_digest)
        .digest(&self.current_receipt_digest)
        .finish()
    }

    /// Statement digest `H("credit-status-statement", transcript)`.
    #[must_use]
    pub fn statement_digest(&self) -> [u8; 32] {
        kagemusha_wallet_digest_v1(Role::CreditStatusStatement, &self.transcript())
    }

    /// Validate the statement's fields.
    ///
    /// # Errors
    ///
    /// Rejects another version, zero bindings, and an incomplete current commitment.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("credit_status.version", self.version)?;
        for (field, digest) in [
            ("credit_status.scheme_id", &self.scheme_id),
            ("credit_status.relation_id", &self.relation_id),
            ("credit_status.asset_digest", &self.asset_digest),
            ("credit_status.receiver_wallet_id", &self.receiver_wallet_id),
            (
                "credit_status.receiver_credential_digest",
                &self.receiver_credential_digest,
            ),
            ("credit_status.credit_id", &self.credit_id),
            ("credit_status.payment_digest", &self.payment_digest),
            (
                "credit_status.current_statement_digest",
                &self.current_statement_digest,
            ),
            (
                "credit_status.current_receipt_digest",
                &self.current_receipt_digest,
            ),
        ] {
            require_nonzero_v1(field, digest)?;
        }
        if !self.current.is_complete() {
            return Err(invalid_v1("credit_status.current"));
        }
        Ok(())
    }
}

/// Read-only `CreditStatus` proof of consumed-credit membership (§5.1).
///
/// It advances no state and need not be retained. A proof of absence is not evidence.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletCreditStatusV1"
)]
pub struct KagemushaWalletCreditStatusV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Public statement.
    pub statement: KagemushaWalletCreditStatusStatementV1,
    /// Recursive `CreditStatus` proof.
    pub proof: KagemushaWalletProofV1,
}

impl KagemushaWalletCreditStatusV1 {
    /// Validate the version, the statement and the proof bounds.
    ///
    /// # Errors
    ///
    /// Rejects another version, an invalid statement, and an empty or oversized proof.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("credit_status.version", self.version)?;
        self.statement.validate()?;
        self.proof.validate_credit_status()
    }
}

/// Delivery evidence carried by Credited (§5.1).
// The Status variant carries a second package-sized value; boxing would only add allocations
// to a bounded, short-lived message whose wire shape stays the same.
// TODO(G3 owner decision, design C2): with the provisional 6,016 + 2,000 proof budgets the
// worst-case Status envelope measures 9,999 of 10,000 bytes. The recorded alternative carries
// the current package as `{statement, proof_digest, receipt}` and lets the CreditStatus relation
// verify the current proof recursively; decide before G3 freezes the relation.
#[allow(
    clippy::large_enum_variant,
    reason = "the bounded evidence stays inline in the canonical wire value"
)]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletCreditedEvidenceV1"
)]
pub enum KagemushaWalletCreditedEvidenceV1 {
    /// The complete Receive package that credited the Payment.
    #[codec(index = 1)]
    Receive {
        /// Complete Receive package.
        package: KagemushaWalletPackageV1,
    },
    /// A `CreditStatus` proof against the receiver's current complete package.
    #[codec(index = 2)]
    Status {
        /// Receiver's current complete package.
        current: KagemushaWalletPackageV1,
        /// Read-only membership proof against `current`.
        status: KagemushaWalletCreditStatusV1,
    },
}

impl KagemushaWalletCreditedEvidenceV1 {
    /// Wire and transcript tag.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        match self {
            Self::Receive { .. } => 1,
            Self::Status { .. } => 2,
        }
    }
}

/// Optional delivery evidence for one credit (§5.1).
///
/// `certificates` is exactly the receiver issuer certificate. The payer matches it by
/// `receiver_wallet_id`, never by credential-digest equality.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletCreditedV1"
)]
pub struct KagemushaWalletCreditedV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Credited credit identity.
    pub credit_id: [u8; 32],
    /// Digest of the full canonical Payment that was credited.
    pub payment_digest: [u8; 32],
    /// Receiver's current credential.
    pub receiver_credential: KagemushaWalletCredentialV1,
    /// Receive package or `CreditStatus` evidence.
    pub evidence: KagemushaWalletCreditedEvidenceV1,
    /// Exactly the receiver credential's issuer certificate.
    pub certificates: KagemushaWalletCertificateSetV1,
}

impl KagemushaWalletCreditedV1 {
    /// Evidence from the complete Receive package that credited the Payment.
    ///
    /// # Errors
    ///
    /// Rejects a package that is not a verified Receive and what [`Self::credited_digest`]
    /// rejects.
    pub fn from_receive(
        receiver_credential: KagemushaWalletCredentialV1,
        issuer_certificate: &KagemushaWalletSignerCertificateV1,
        package: KagemushaWalletPackageV1,
    ) -> WalletResult<Self> {
        let KagemushaWalletEffectV1::Receive {
            credit_id, payment, ..
        } = package.statement.effect
        else {
            return Err(invalid_v1("credited.evidence"));
        };
        let credited = Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            credit_id,
            payment_digest: payment,
            receiver_credential,
            evidence: KagemushaWalletCreditedEvidenceV1::Receive { package },
            certificates: KagemushaWalletCertificateSetV1::new(vec![*issuer_certificate])?,
        };
        credited.credited_digest()?;
        Ok(credited)
    }

    /// Evidence from a `CreditStatus` proof against the receiver's current package.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::credited_digest`] rejects.
    pub fn from_status(
        receiver_credential: KagemushaWalletCredentialV1,
        issuer_certificate: &KagemushaWalletSignerCertificateV1,
        current: KagemushaWalletPackageV1,
        status: KagemushaWalletCreditStatusV1,
    ) -> WalletResult<Self> {
        let credited = Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            credit_id: status.statement.credit_id,
            payment_digest: status.statement.payment_digest,
            receiver_credential,
            evidence: KagemushaWalletCreditedEvidenceV1::Status { current, status },
            certificates: KagemushaWalletCertificateSetV1::new(vec![*issuer_certificate])?,
        };
        credited.credited_digest()?;
        Ok(credited)
    }

    /// Fully validate the evidence and return its exact `credited` transcript (design §4.4
    /// and C5):
    /// `LE16 version || credit_id || payment_digest || receiver_credential_digest || u8 tag ||
    /// package_digest || status_statement_digest or zero || status_proof_digest or zero ||
    /// certificate-set digest`.
    ///
    /// # Errors
    ///
    /// Rejects another version, zero identities, an invalid receiver credential, a certificate
    /// set other than exactly its issuer certificate; for Receive, a package that is not the
    /// matching Receive or does not verify; for Status, an invalid status, a statement for
    /// another credit, Payment, scheme, relation, asset, wallet or credential, and current
    /// bindings that differ from the current package, which must verify.
    pub fn transcript(&self) -> WalletResult<Vec<u8>> {
        require_version_v1("credited.version", self.version)?;
        require_nonzero_v1("credited.credit_id", &self.credit_id)?;
        require_nonzero_v1("credited.payment_digest", &self.payment_digest)?;
        let credential = &self.receiver_credential;
        credential.validate()?;
        require_exact_certificates_v1(
            &self.certificates,
            &credential.body.scheme_id,
            &[(
                credential.body.issuer_certificate,
                KagemushaWalletSignerRoleV1::Enrollment,
            )],
        )?;
        let credential_digest = credential.credential_digest();
        let (package_digest, status_statement, status_proof) = match &self.evidence {
            KagemushaWalletCreditedEvidenceV1::Receive { package } => {
                match package.statement.effect {
                    KagemushaWalletEffectV1::Receive {
                        credit_id, payment, ..
                    } => {
                        if credit_id != self.credit_id {
                            return Err(invalid_v1("credited.receive.credit_id"));
                        }
                        if payment != self.payment_digest {
                            return Err(invalid_v1("credited.receive.payment"));
                        }
                    }
                    _ => return Err(invalid_v1("credited.evidence")),
                }
                (package.verify(credential)?.package, [0; 32], [0; 32])
            }
            KagemushaWalletCreditedEvidenceV1::Status { current, status } => {
                status.validate()?;
                let claim = &status.statement;
                let statement = &current.statement;
                for (field, matches) in [
                    ("credit_status.credit_id", claim.credit_id == self.credit_id),
                    (
                        "credit_status.payment_digest",
                        claim.payment_digest == self.payment_digest,
                    ),
                    (
                        "credit_status.asset_digest",
                        claim.asset_digest == statement.asset_digest,
                    ),
                    (
                        "credit_status.receiver_wallet_id",
                        claim.receiver_wallet_id == credential.body.wallet_id,
                    ),
                    (
                        "credit_status.receiver_credential_digest",
                        claim.receiver_credential_digest == credential_digest,
                    ),
                    (
                        "credit_status.current",
                        claim.current == statement.successor,
                    ),
                    (
                        "credit_status.current_sequence",
                        claim.current_sequence == statement.sequence,
                    ),
                    (
                        "credit_status.current_statement_digest",
                        claim.current_statement_digest == statement.statement_digest(),
                    ),
                ] {
                    if !matches {
                        return Err(invalid_v1(field));
                    }
                }
                require_scheme_v1(
                    "credit_status.scheme_id",
                    &claim.scheme_id,
                    &statement.scheme_id,
                )?;
                require_scheme_v1(
                    "credit_status.relation_id",
                    &claim.relation_id,
                    &statement.relation_id,
                )?;
                let digests = current.verify(credential)?;
                if claim.current_receipt_digest != digests.receipt {
                    return Err(invalid_v1("credit_status.current_receipt_digest"));
                }
                (
                    digests.package,
                    claim.statement_digest(),
                    status.proof.proof_digest(),
                )
            }
        };
        Ok(
            WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1)
                .u16(self.version)
                .digest(&self.credit_id)
                .digest(&self.payment_digest)
                .digest(&credential_digest)
                .u8(self.evidence.tag())
                .digest(&package_digest)
                .digest(&status_statement)
                .digest(&status_proof)
                .digest(&self.certificates.digest()?)
                .finish(),
        )
    }

    /// Fully validate the evidence and return `credited_digest = H("credited", transcript)`.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::transcript`] rejects.
    pub fn credited_digest(&self) -> WalletResult<[u8; 32]> {
        Ok(kagemusha_wallet_digest_v1(
            Role::Credited,
            &self.transcript()?,
        ))
    }

    /// Fully validate the evidence.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::credited_digest`] rejects.
    pub fn validate(&self) -> WalletResult<()> {
        self.credited_digest().map(|_| ())
    }

    /// Validate the evidence and verify the receiver credential, its certificate and the
    /// relation under `scheme`.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::credited_digest`] rejects, another scheme or relation, and issuer
    /// or root signatures that do not verify.
    pub fn verify(&self, scheme: &KagemushaWalletSchemeV1) -> WalletResult<[u8; 32]> {
        let digest = self.credited_digest()?;
        verify_credential_with_set_v1(&self.receiver_credential, scheme, &self.certificates)?;
        let package = match &self.evidence {
            KagemushaWalletCreditedEvidenceV1::Receive { package } => package,
            KagemushaWalletCreditedEvidenceV1::Status { current, .. } => current,
        };
        package.statement.validate_for_scheme(scheme)?;
        Ok(digest)
    }

    /// `ArchiveSent` effect of the payer's retained `payment` and its pending leaf (design C5).
    ///
    /// # Errors
    ///
    /// Rejects invalid evidence or Payment, evidence for another Payment digest or credit, a
    /// pending leaf that is not the Payment's, and evidence from another receiver wallet,
    /// scheme or asset.
    pub fn archive_sent_effect(
        &self,
        payment: &KagemushaWalletPaymentV1,
        pending: &KagemushaWalletPendingOutgoingLeafV1,
    ) -> WalletResult<KagemushaWalletEffectV1> {
        let credited = self.credited_digest()?;
        let expected = payment.pending_outgoing_leaf()?;
        let request = &payment.request.body;
        let receiver = &self.receiver_credential.body;
        if self.payment_digest != payment.payment_digest()? {
            return Err(invalid_v1("credited.payment_digest"));
        }
        if self.credit_id != expected.credit_id {
            return Err(invalid_v1("credited.credit_id"));
        }
        if *pending != expected {
            return Err(invalid_v1("pending_outgoing"));
        }
        require_scheme_v1(
            "credited.scheme_id",
            &receiver.scheme_id,
            &request.scheme_id,
        )?;
        if receiver.asset_digest != request.asset_digest {
            return Err(invalid_v1("credited.asset_digest"));
        }
        if receiver.wallet_id != request.receiver_wallet_id {
            return Err(invalid_v1("credited.receiver_wallet_id"));
        }
        Ok(KagemushaWalletEffectV1::ArchiveSent {
            credit_id: self.credit_id,
            credited,
        })
    }
}

// ---------------------------------------------------------------------------------------
// Session control (§§5.1, 8, design §4.5 and C8)
// ---------------------------------------------------------------------------------------

/// Kind of one nonmonetary session control.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletSessionControlKindV1"
)]
pub enum KagemushaWalletSessionControlKindV1 {
    /// Setup declined before Send; carries a reason code.
    #[codec(index = 1)]
    SetupDeclined,
    /// The offered scheme is not supported; always unsigned and stateless.
    #[codec(index = 2)]
    UnsupportedScheme,
    /// Receive postponed (for example for capacity); carries the credit and a reason code.
    #[codec(index = 3)]
    ReceiveDeferred,
    /// Session closed.
    #[codec(index = 4)]
    Close,
}

impl KagemushaWalletSessionControlKindV1 {
    /// Every kind, in tag order.
    pub const ALL: [Self; 4] = [
        Self::SetupDeclined,
        Self::UnsupportedScheme,
        Self::ReceiveDeferred,
        Self::Close,
    ];

    /// Transcript tag; equal to the Norito wire tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::SetupDeclined => 1,
            Self::UnsupportedScheme => 2,
            Self::ReceiveDeferred => 3,
            Self::Close => 4,
        }
    }

    /// Width of the kind-dependent fields this kind uses.
    #[must_use]
    pub const fn fields_bytes(self) -> usize {
        match self {
            Self::SetupDeclined => SETUP_DECLINED_FIELDS_BYTES,
            Self::UnsupportedScheme => UNSUPPORTED_SCHEME_FIELDS_BYTES,
            Self::ReceiveDeferred => RECEIVE_DEFERRED_FIELDS_BYTES,
            Self::Close => CLOSE_FIELDS_BYTES,
        }
    }
}

/// Authentication of a session control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletSessionAuthV1"
)]
pub enum KagemushaWalletSessionAuthV1 {
    /// Unsigned: `UnsupportedScheme`, or `SetupDeclined` before the sender's own Offer or
    /// Request in the session.
    #[codec(index = 0)]
    Unsigned,
    /// Signed by the sender payment key under `session-control-body`.
    #[codec(index = 1)]
    Signed {
        /// Payment-key signature.
        signature: KagemushaDeviceSignatureV1,
    },
}

impl KagemushaWalletSessionAuthV1 {
    /// Wire tag.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        match self {
            Self::Unsigned => 0,
            Self::Signed { .. } => 1,
        }
    }
}

/// Nonmonetary session control; receivers drop invalid controls (§§5.1, 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletSessionControlV1"
)]
pub struct KagemushaWalletSessionControlV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Session scheme; for `UnsupportedScheme`, the declined scheme.
    pub scheme_id: [u8; 32],
    /// Session asset scope digest.
    pub asset_digest: [u8; 32],
    /// Sending wallet; zero only for `UnsupportedScheme` (no wallet in that scheme).
    pub sender_wallet_id: [u8; 32],
    /// Peer wallet; zero when unknown.
    pub peer_wallet_id: [u8; 32],
    /// Session nonce.
    pub session_nonce: [u8; 32],
    /// Control kind.
    pub kind: KagemushaWalletSessionControlKindV1,
    /// Reason code of `SetupDeclined` and `ReceiveDeferred`; zero otherwise.
    pub reason: u16,
    /// Deferred credit of `ReceiveDeferred`; zero otherwise.
    pub credit_id: [u8; 32],
    /// Authentication.
    pub auth: KagemushaWalletSessionAuthV1,
}

impl KagemushaWalletSessionControlV1 {
    /// Stateless unsigned `UnsupportedScheme` reply to an Offer whose scheme the receiver does
    /// not support (§8).
    ///
    /// It names the declined scheme, asset and session nonce of `offer` and the offering payer
    /// as its peer; the sender wallet is zero because the receiver has no wallet in that scheme.
    /// Obtain `offer` from [`KagemushaWalletEnvelopeV1::decode_offered_scheme`].
    ///
    /// # Errors
    ///
    /// Rejects an invalid Offer body.
    pub fn unsupported_scheme(offer: &KagemushaWalletOfferBodyV1) -> WalletResult<Self> {
        offer.validate()?;
        let control = Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: offer.scheme_id,
            asset_digest: offer.asset_digest,
            sender_wallet_id: [0; 32],
            peer_wallet_id: offer.payer_wallet_id,
            session_nonce: offer.session_nonce,
            kind: KagemushaWalletSessionControlKindV1::UnsupportedScheme,
            reason: 0,
            credit_id: [0; 32],
            auth: KagemushaWalletSessionAuthV1::Unsigned,
        };
        control.validate()?;
        Ok(control)
    }

    /// Exact `session-control-body` transcript.
    #[must_use]
    pub fn transcript(&self) -> Vec<u8> {
        WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_SESSION_CONTROL_BODY_TRANSCRIPT_BYTES_V1)
            .u16(self.version)
            .digest(&self.scheme_id)
            .digest(&self.asset_digest)
            .digest(&self.sender_wallet_id)
            .digest(&self.peer_wallet_id)
            .digest(&self.session_nonce)
            .u8(self.kind.tag())
            .u16(self.reason)
            .digest(&self.credit_id)
            .finish()
    }

    /// Signed body digest `H("session-control-body", transcript)`.
    #[must_use]
    pub fn body_digest(&self) -> [u8; 32] {
        kagemusha_wallet_digest_v1(Role::SessionControlBody, &self.transcript())
    }

    /// Exact ECDSA message the sender payment key signs.
    #[must_use]
    pub fn signing_message(&self) -> Vec<u8> {
        kagemusha_wallet_preimage_v1(Role::SessionControlBody, &self.transcript())
    }

    /// Freeze the sender payment-key signature over this control.
    ///
    /// # Errors
    ///
    /// Rejects an invalid control, an `UnsupportedScheme` control, a control for another
    /// wallet, scheme or asset than `sender_credential`, or a signature that does not verify.
    pub fn sign(
        mut self,
        sender_credential: &KagemushaWalletCredentialV1,
        signer_output: KagemushaWalletSignerOutputV1<'_>,
    ) -> WalletResult<Self> {
        if self.kind == KagemushaWalletSessionControlKindV1::UnsupportedScheme {
            return Err(invalid_v1("session_control.auth"));
        }
        self.auth = KagemushaWalletSessionAuthV1::Unsigned;
        self.validate()?;
        self.require_sender(sender_credential)?;
        let signature = kagemusha_wallet_freeze_signature_v1(
            &sender_credential.body.payment_key,
            Role::SessionControlBody,
            &self.transcript(),
            signer_output,
        )?;
        self.auth = KagemushaWalletSessionAuthV1::Signed { signature };
        Ok(self)
    }

    fn require_sender(&self, credential: &KagemushaWalletCredentialV1) -> WalletResult<()> {
        credential.validate()?;
        let sender = &credential.body;
        require_scheme_v1(
            "session_control.scheme_id",
            &self.scheme_id,
            &sender.scheme_id,
        )?;
        if self.asset_digest != sender.asset_digest {
            return Err(invalid_v1("session_control.asset_digest"));
        }
        if self.sender_wallet_id != sender.wallet_id {
            return Err(invalid_v1("session_control.sender_wallet_id"));
        }
        Ok(())
    }

    /// Validate the control's structure.
    ///
    /// # Errors
    ///
    /// Rejects another version, zero bindings, a peer equal to the sender, kind fields that
    /// are set when unused or missing when used, and a signed `UnsupportedScheme`.
    pub fn validate(&self) -> WalletResult<()> {
        use KagemushaWalletSessionControlKindV1 as Kind;
        require_version_v1("session_control.version", self.version)?;
        require_nonzero_v1("session_control.scheme_id", &self.scheme_id)?;
        require_nonzero_v1("session_control.asset_digest", &self.asset_digest)?;
        require_nonzero_v1("session_control.session_nonce", &self.session_nonce)?;
        if self.kind != Kind::UnsupportedScheme {
            require_nonzero_v1("session_control.sender_wallet_id", &self.sender_wallet_id)?;
        }
        if !is_zero_v1(&self.peer_wallet_id) && self.peer_wallet_id == self.sender_wallet_id {
            return Err(invalid_v1("session_control.peer_wallet_id"));
        }
        let uses_reason = matches!(self.kind, Kind::SetupDeclined | Kind::ReceiveDeferred);
        if !uses_reason && self.reason != 0 {
            return Err(invalid_v1("session_control.reason"));
        }
        if (self.kind == Kind::ReceiveDeferred) == is_zero_v1(&self.credit_id) {
            return Err(invalid_v1("session_control.credit_id"));
        }
        match self.auth {
            KagemushaWalletSessionAuthV1::Unsigned => Ok(()),
            KagemushaWalletSessionAuthV1::Signed { signature } => {
                if self.kind == Kind::UnsupportedScheme {
                    return Err(invalid_v1("session_control.auth"));
                }
                signature.validate()?;
                Ok(())
            }
        }
    }

    /// Verify the control against the sender credential learned in the same session (design
    /// C8).
    ///
    /// Pass the credential of the sender's Offer (payer) or Request (receiver) in this
    /// session, or `None` when the sender has sent neither. Unsigned controls are accepted only
    /// for `UnsupportedScheme`, and for `SetupDeclined` before the sender's key is known.
    ///
    /// # Errors
    ///
    /// Rejects an invalid control, a signed control whose key is unknown, an unsigned control
    /// that must be signed, a control for another wallet, scheme or asset than the credential,
    /// and a signature that does not verify.
    pub fn verify(
        &self,
        sender_credential: Option<&KagemushaWalletCredentialV1>,
    ) -> WalletResult<()> {
        use KagemushaWalletSessionControlKindV1 as Kind;
        self.validate()?;
        match (self.auth, sender_credential) {
            (KagemushaWalletSessionAuthV1::Signed { signature }, Some(credential)) => {
                self.require_sender(credential)?;
                kagemusha_wallet_verify_signature_v1(
                    &credential.body.payment_key,
                    Role::SessionControlBody,
                    &self.transcript(),
                    &signature,
                )
            }
            (KagemushaWalletSessionAuthV1::Signed { .. }, None) => {
                Err(invalid_v1("session_control.sender_key"))
            }
            (KagemushaWalletSessionAuthV1::Unsigned, _) if self.kind == Kind::UnsupportedScheme => {
                Ok(())
            }
            (KagemushaWalletSessionAuthV1::Unsigned, None) if self.kind == Kind::SetupDeclined => {
                Ok(())
            }
            (KagemushaWalletSessionAuthV1::Unsigned, _) => Err(invalid_v1("session_control.auth")),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Policy data (§§3.3, 7, design §4.6 and C8)
// ---------------------------------------------------------------------------------------

/// One nonmonetary scheme-scoped policy item carried between peers.
///
/// The signed blacklist is not a policy item and never travels between peers: a wallet
/// downloads it only while online, from the issuer or ledger, as one standalone
/// [`KagemushaWalletBlacklistV1`](super::KagemushaWalletBlacklistV1) frame. Wire tag 4 is
/// unused.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletPolicyDataItemV1"
)]
pub enum KagemushaWalletPolicyDataItemV1 {
    /// Signed scheme policy.
    #[codec(index = 1)]
    SchemePolicy {
        /// Signed scheme policy.
        policy: KagemushaWalletSchemePolicyV1,
    },
    /// Signed fee schedule.
    #[codec(index = 2)]
    FeeSchedule {
        /// Signed fee schedule.
        schedule: KagemushaWalletFeeScheduleV1,
    },
    /// Root-signed signer certificates; never empty.
    #[codec(index = 3)]
    Certificates {
        /// Certificate set.
        certificates: KagemushaWalletCertificateSetV1,
    },
}

impl KagemushaWalletPolicyDataItemV1 {
    /// Wire tag.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        match self {
            Self::SchemePolicy { .. } => 1,
            Self::FeeSchedule { .. } => 2,
            Self::Certificates { .. } => 3,
        }
    }
}

/// Peer-carried policy data: one scheme-scoped item under explicit scheme and asset fields.
///
/// Scheme-scoped objects are exempt from wallet binding: they are signed by scheme
/// authorities and carry no wallet authority. Applying an item is a separate `RefreshPolicy`.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletPolicyDataV1"
)]
pub struct KagemushaWalletPolicyDataV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Scheme of the item.
    pub scheme_id: [u8; 32],
    /// Session asset scope digest; asset-scoped items must equal it.
    pub asset_digest: [u8; 32],
    /// Policy item.
    pub item: KagemushaWalletPolicyDataItemV1,
}

impl KagemushaWalletPolicyDataV1 {
    /// Validate the item and its scheme and asset bindings (design C8).
    ///
    /// # Errors
    ///
    /// Rejects another version, zero bindings, an invalid item, an item for another scheme or
    /// asset, and an empty certificate set.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("policy_data.version", self.version)?;
        require_nonzero_v1("policy_data.scheme_id", &self.scheme_id)?;
        require_nonzero_v1("policy_data.asset_digest", &self.asset_digest)?;
        match &self.item {
            KagemushaWalletPolicyDataItemV1::SchemePolicy { policy } => {
                policy.validate()?;
                self.require_scope(&policy.body.scheme_id, Some(&policy.body.asset_digest))
            }
            KagemushaWalletPolicyDataItemV1::FeeSchedule { schedule } => {
                schedule.validate()?;
                self.require_scope(&schedule.body.scheme_id, Some(&schedule.body.asset_digest))
            }
            KagemushaWalletPolicyDataItemV1::Certificates { certificates } => {
                certificates.validate()?;
                if certificates.is_empty() {
                    return Err(invalid_v1("policy_data.certificates"));
                }
                for certificate in &certificates.certificates {
                    self.require_scope(&certificate.body.scheme_id, None)?;
                }
                Ok(())
            }
        }
    }

    fn require_scope(
        &self,
        scheme_id: &[u8; 32],
        asset_digest: Option<&[u8; 32]>,
    ) -> WalletResult<()> {
        require_scheme_v1("policy_data.scheme_id", scheme_id, &self.scheme_id)?;
        if asset_digest.is_some_and(|asset_digest| *asset_digest != self.asset_digest) {
            return Err(invalid_v1("policy_data.asset_digest"));
        }
        Ok(())
    }

    /// Validate the item and verify its signatures under `scheme`.
    ///
    /// Signed policy items select their signer from `known_certificates` by digest;
    /// certificate items are verified under the scheme root.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::validate`] rejects, another scheme, a missing or wrong-role signer
    /// certificate, and signatures that do not verify.
    pub fn verify(
        &self,
        scheme: &KagemushaWalletSchemeV1,
        known_certificates: &[KagemushaWalletSignerCertificateV1],
    ) -> WalletResult<()> {
        self.validate()?;
        require_scheme_v1(
            "policy_data.scheme_id",
            &self.scheme_id,
            &scheme.scheme_id(),
        )?;
        let signer = |digest: &[u8; 32]| {
            known_certificates
                .iter()
                .find(|certificate| certificate.certificate_digest() == *digest)
                .ok_or_else(|| invalid_v1("certificates.missing"))
        };
        match &self.item {
            KagemushaWalletPolicyDataItemV1::SchemePolicy { policy } => {
                policy.verify(scheme, signer(&policy.body.signer_certificate)?)
            }
            KagemushaWalletPolicyDataItemV1::FeeSchedule { schedule } => {
                schedule.verify(scheme, signer(&schedule.body.signer_certificate)?)
            }
            KagemushaWalletPolicyDataItemV1::Certificates { certificates } => {
                certificates.verify(scheme)
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Envelope, per-kind bounds and `kgm1:` text (§8, design §4.7)
// ---------------------------------------------------------------------------------------

/// One peer message; its tag selects the per-kind frame bound.
// The Payment carries a Request and a package; boxing would only add allocations to bounded,
// short-lived carrier values whose wire shape stays the same.
#[allow(
    clippy::large_enum_variant,
    reason = "bounded messages stay inline in the canonical wire value"
)]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletMessageV1"
)]
pub enum KagemushaWalletMessageV1 {
    /// Payer session hint.
    #[codec(index = 1)]
    Offer {
        /// Offer.
        offer: KagemushaWalletOfferV1,
    },
    /// Receiver setup quote.
    #[codec(index = 2)]
    Request {
        /// Request.
        request: KagemushaWalletRequestV1,
    },
    /// Complete committed Payment.
    #[codec(index = 3)]
    Payment {
        /// Payment.
        payment: KagemushaWalletPaymentV1,
    },
    /// Delivery evidence.
    #[codec(index = 4)]
    Credited {
        /// Credited evidence.
        credited: KagemushaWalletCreditedV1,
    },
    /// Nonmonetary session control.
    #[codec(index = 5)]
    SessionControl {
        /// Session control.
        control: KagemushaWalletSessionControlV1,
    },
    /// Nonmonetary policy data.
    #[codec(index = 6)]
    PolicyData {
        /// Policy data.
        data: KagemushaWalletPolicyDataV1,
    },
}

impl KagemushaWalletMessageV1 {
    /// Wire tag.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        match self {
            Self::Offer { .. } => 1,
            Self::Request { .. } => 2,
            Self::Payment { .. } => 3,
            Self::Credited { .. } => 4,
            Self::SessionControl { .. } => 5,
            Self::PolicyData { .. } => 6,
        }
    }

    /// Maximum complete envelope frame of this kind (§8).
    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        match self {
            Self::Offer { .. } | Self::SessionControl { .. } => {
                KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1
            }
            Self::Request { .. }
            | Self::Payment { .. }
            | Self::Credited { .. }
            | Self::PolicyData { .. } => KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
        }
    }

    /// Top-level version field of the message.
    #[must_use]
    pub const fn version(&self) -> u16 {
        match self {
            Self::Offer { offer } => offer.body.version,
            Self::Request { request } => request.body.version,
            Self::Payment { payment } => payment.version,
            Self::Credited { credited } => credited.version,
            Self::SessionControl { control } => control.version,
            Self::PolicyData { data } => data.version,
        }
    }

    /// Scheme field checked at decode (design C5): the body scheme of Offer and Request, the
    /// Request scheme of Payment, the receiver credential scheme of Credited, and the message's
    /// own scheme field otherwise.
    #[must_use]
    pub const fn scheme_id(&self) -> &[u8; 32] {
        match self {
            Self::Offer { offer } => &offer.body.scheme_id,
            Self::Request { request } => &request.body.scheme_id,
            Self::Payment { payment } => &payment.request.body.scheme_id,
            Self::Credited { credited } => &credited.receiver_credential.body.scheme_id,
            Self::SessionControl { control } => &control.scheme_id,
            Self::PolicyData { data } => &data.scheme_id,
        }
    }

    /// Validate the message's self-contained rules, including every signature whose key the
    /// message carries.
    ///
    /// # Errors
    ///
    /// Rejects what the message type's own validation rejects.
    pub fn validate(&self) -> WalletResult<()> {
        match self {
            Self::Offer { offer } => offer.validate(),
            Self::Request { request } => request.validate(),
            Self::Payment { payment } => payment.validate(),
            Self::Credited { credited } => credited.validate(),
            Self::SessionControl { control } => control.validate(),
            Self::PolicyData { data } => data.validate(),
        }
    }
}

/// Reject a complete envelope frame above its message kind's bound.
fn require_message_bound_v1(message: &KagemushaWalletMessageV1, len: usize) -> WalletResult<()> {
    let max = message.max_bytes();
    if len > max {
        return Err(KagemushaWalletValidationErrorV1::EncodedSizeExceeded { actual: len, max });
    }
    Ok(())
}

/// The single canonical Norito V1 envelope of every peer message (§8).
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletEnvelopeV1"
)]
pub struct KagemushaWalletEnvelopeV1 {
    /// Wire version; exactly [`KAGEMUSHA_WALLET_VERSION_V1`].
    pub version: u16,
    /// Carried message.
    pub message: KagemushaWalletMessageV1,
}

impl KagemushaWalletEnvelopeV1 {
    /// Wrap `message` in a V1 envelope.
    #[must_use]
    pub const fn new(message: KagemushaWalletMessageV1) -> Self {
        Self {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            message,
        }
    }

    /// Validate the envelope and its message.
    ///
    /// # Errors
    ///
    /// Rejects another envelope or message version and what the message rejects.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("envelope.version", self.version)?;
        require_version_v1("message.version", self.message.version())?;
        self.message.validate()
    }

    /// Validate and encode the canonical frame within its per-kind bound.
    ///
    /// # Errors
    ///
    /// Rejects an invalid envelope or a frame above its kind's bound.
    pub fn to_canonical_bytes(&self) -> WalletResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, self.message.max_bytes())
    }

    /// Decode one canonical frame up to, but not including, the expected-scheme check: byte
    /// cap, canonical decode, every version field, then the per-kind bound (design §0).
    fn decode_bounded(bytes: &[u8]) -> WalletResult<Self> {
        let envelope: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?;
        envelope.require_versions()?;
        require_message_bound_v1(&envelope.message, bytes.len())?;
        Ok(envelope)
    }

    /// Decode one canonical envelope frame for `expected_scheme_id` (design §0 order).
    ///
    /// # Errors
    ///
    /// Rejects, in order, a frame above the largest bound, a noncanonical frame, another
    /// version in any envelope, message or nested field, a frame above its kind's bound,
    /// another scheme, and what [`Self::validate`] rejects.
    pub fn decode_canonical(bytes: &[u8], expected_scheme_id: &[u8; 32]) -> WalletResult<Self> {
        let envelope = Self::decode_bounded(bytes)?;
        require_scheme_v1(
            "envelope.scheme_id",
            envelope.message.scheme_id(),
            expected_scheme_id,
        )?;
        envelope.validate()?;
        Ok(envelope)
    }

    /// Decode an Offer envelope only far enough to decline its scheme (§8: "an unknown scheme
    /// can be declined during Offer").
    ///
    /// Runs the byte cap, canonical decoding, every version field and the per-kind bound in the
    /// order of [`Self::decode_canonical`], requires an Offer, and returns its structurally
    /// valid body without the expected-scheme check. The body is unauthenticated: the payer
    /// credential and signature are not verified, because a receiver that does not support the
    /// scheme cannot verify them. It confers nothing beyond the stateless
    /// [`KagemushaWalletSessionControlV1::unsupported_scheme`] reply; a supported scheme must be
    /// decoded with [`Self::decode_canonical`].
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::decode_canonical`] rejects before its scheme check, a message other
    /// than an Offer, and an invalid Offer body.
    pub fn decode_offered_scheme(bytes: &[u8]) -> WalletResult<KagemushaWalletOfferBodyV1> {
        let envelope = Self::decode_bounded(bytes)?;
        let KagemushaWalletMessageV1::Offer { offer } = envelope.message else {
            return Err(invalid_v1("envelope.message"));
        };
        offer.body.validate()?;
        Ok(offer.body)
    }

    /// Encode the `kgm1:` text form: the prefix and unpadded base64url of the canonical frame.
    ///
    /// # Errors
    ///
    /// Rejects what [`Self::to_canonical_bytes`] rejects.
    pub fn to_text(&self) -> WalletResult<String> {
        Ok(kagemusha_wallet_text_encode_v1(&self.to_canonical_bytes()?))
    }

    /// Strictly decode the `kgm1:` text form and then the canonical frame.
    ///
    /// # Errors
    ///
    /// Rejects what [`kagemusha_wallet_text_decode_v1`] and [`Self::decode_canonical`]
    /// reject.
    pub fn from_text(text: &str, expected_scheme_id: &[u8; 32]) -> WalletResult<Self> {
        Self::decode_canonical(&kagemusha_wallet_text_decode_v1(text)?, expected_scheme_id)
    }

    /// Strictly decode the `kgm1:` text form of an Offer only far enough to decline its scheme.
    ///
    /// # Errors
    ///
    /// Rejects what [`kagemusha_wallet_text_decode_v1`] and [`Self::decode_offered_scheme`]
    /// reject.
    pub fn offered_scheme_from_text(text: &str) -> WalletResult<KagemushaWalletOfferBodyV1> {
        Self::decode_offered_scheme(&kagemusha_wallet_text_decode_v1(text)?)
    }
}

/// `kgm1:` text of one canonical frame: the prefix and unpadded base64url.
#[must_use]
pub fn kagemusha_wallet_text_encode_v1(frame: &[u8]) -> String {
    let mut text = String::with_capacity(
        KAGEMUSHA_WALLET_TEXT_PREFIX_V1
            .len()
            .saturating_add(frame.len().saturating_mul(4) / 3)
            .saturating_add(4),
    );
    text.push_str(KAGEMUSHA_WALLET_TEXT_PREFIX_V1);
    URL_SAFE_NO_PAD.encode_string(frame, &mut text);
    text
}

/// Strictly decode `kgm1:` text into canonical frame bytes (§8, design §4.7).
///
/// The text must not exceed the largest text bound, must start with the prefix, use only the
/// base64url alphabet without padding or whitespace, have a length that is not `1 mod 4`, and
/// re-encode to itself.
///
/// # Errors
///
/// Rejects oversized text and any other form.
pub fn kagemusha_wallet_text_decode_v1(text: &str) -> WalletResult<Vec<u8>> {
    if text.len() > KAGEMUSHA_WALLET_MESSAGE_TEXT_MAX_BYTES_V1 {
        return Err(KagemushaWalletValidationErrorV1::EncodedSizeExceeded {
            actual: text.len(),
            max: KAGEMUSHA_WALLET_MESSAGE_TEXT_MAX_BYTES_V1,
        });
    }
    let body = text
        .strip_prefix(KAGEMUSHA_WALLET_TEXT_PREFIX_V1)
        .ok_or_else(|| invalid_v1("text.prefix"))?;
    if body.is_empty() {
        return Err(invalid_v1("text.body"));
    }
    if !body
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(invalid_v1("text.alphabet"));
    }
    if body.len() % 4 == 1 {
        return Err(invalid_v1("text.length"));
    }
    let frame = URL_SAFE_NO_PAD
        .decode(body.as_bytes())
        .map_err(|_| invalid_v1("text.base64url"))?;
    if URL_SAFE_NO_PAD.encode(&frame) != body {
        return Err(invalid_v1("text.base64url"));
    }
    Ok(frame)
}

// ---------------------------------------------------------------------------------------
// Version fields (design §0 decode order)
// ---------------------------------------------------------------------------------------

impl WalletVersionsV1 for KagemushaWalletOfferV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("offer.version", self.body.version)?;
        self.payer_credential.require_versions()?;
        self.certificates.require_versions()
    }
}

impl WalletVersionsV1 for KagemushaWalletFeeScheduleSlotV1 {
    fn require_versions(&self) -> WalletResult<()> {
        self.schedule()
            .map_or(Ok(()), WalletVersionsV1::require_versions)
    }
}

impl WalletVersionsV1 for KagemushaWalletRequestV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("request.version", self.body.version)?;
        self.receiver_credential.require_versions()?;
        self.fee_schedule.require_versions()?;
        self.certificates.require_versions()
    }
}

impl WalletVersionsV1 for KagemushaWalletPaymentV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("payment.version", self.version)?;
        self.request.require_versions()?;
        self.payer_credential.require_versions()?;
        self.send.require_versions()?;
        self.certificates.require_versions()
    }
}

impl WalletVersionsV1 for KagemushaWalletCreditStatusV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("credit_status.version", self.version)?;
        require_version_v1("credit_status.version", self.statement.version)
    }
}

impl WalletVersionsV1 for KagemushaWalletCreditedEvidenceV1 {
    fn require_versions(&self) -> WalletResult<()> {
        match self {
            Self::Receive { package } => package.require_versions(),
            Self::Status { current, status } => {
                current.require_versions()?;
                status.require_versions()
            }
        }
    }
}

impl WalletVersionsV1 for KagemushaWalletCreditedV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("credited.version", self.version)?;
        self.receiver_credential.require_versions()?;
        self.evidence.require_versions()?;
        self.certificates.require_versions()
    }
}

impl WalletVersionsV1 for KagemushaWalletSessionControlV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("session_control.version", self.version)
    }
}

impl WalletVersionsV1 for KagemushaWalletPolicyDataItemV1 {
    fn require_versions(&self) -> WalletResult<()> {
        match self {
            Self::SchemePolicy { policy } => policy.require_versions(),
            Self::FeeSchedule { schedule } => schedule.require_versions(),
            Self::Certificates { certificates } => certificates.require_versions(),
        }
    }
}

impl WalletVersionsV1 for KagemushaWalletPolicyDataV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("policy_data.version", self.version)?;
        self.item.require_versions()
    }
}

impl WalletVersionsV1 for KagemushaWalletMessageV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("message.version", self.version())?;
        match self {
            Self::Offer { offer } => offer.require_versions(),
            Self::Request { request } => request.require_versions(),
            Self::Payment { payment } => payment.require_versions(),
            Self::Credited { credited } => credited.require_versions(),
            Self::SessionControl { control } => control.require_versions(),
            Self::PolicyData { data } => data.require_versions(),
        }
    }
}

impl WalletVersionsV1 for KagemushaWalletEnvelopeV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("envelope.version", self.version)?;
        self.message.require_versions()
    }
}
