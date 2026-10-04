//! Domain-separated digests, signing preimages and signature freezing (§8, design §1).

use p256::ecdsa::Signature as P256Signature;
use sha2::{Digest as _, Sha256};

use super::{KagemushaWalletValidationErrorV1, WalletResult};
use crate::kagemusha::{KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1};

#[cfg(test)]
#[path = "digest_tests.rs"]
mod digest_tests;

/// Prefix of every KAGEMUSHA wallet V1 digest preimage.
pub const KAGEMUSHA_WALLET_DIGEST_PREFIX_V1: &[u8] = b"iroha:kagemusha:wallet:v1:";
/// Exact body bytes hashed by a signed-object digest: `e (32) || signature (64)`.
pub const KAGEMUSHA_WALLET_SIGNED_OBJECT_TRANSCRIPT_BYTES_V1: usize = 96;

/// Exact role label of one domain-separated KAGEMUSHA wallet V1 digest.
///
/// A `*-body` role names the signed transcript of an object; the matching role without the
/// suffix names that signed object's digest `H(role, e || signature)`. Map leaves are not
/// SHA roles: their Poseidon domains belong to the recursive map owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KagemushaWalletDigestRoleV1 {
    /// `scheme`: scheme identity (§2.1).
    Scheme,
    /// `relation`: frozen relation identity (§2.1).
    Relation,
    /// `provider-contract`: Advance contract and receipt format identity (§2.1).
    ProviderContract,
    /// `asset-scope`: asset incarnation and scale (§2.1).
    AssetScope,
    /// `account`: canonical domainless `AccountId` frame.
    Account,
    /// `enrollment-challenge`: issuer enrollment challenge (§2.3).
    EnrollmentChallenge,
    /// `enrollment-id`: enrollment incarnation identity (§2.3).
    EnrollmentId,
    /// `enrollment-key-binding`: App Attest enrollment assertion client data (§2.3).
    EnrollmentKeyBinding,
    /// `wallet-id`: wallet incarnation identity (§2.3).
    WalletId,
    /// `certificate-body`: scheme-root-signed signer certificate transcript (§2.2).
    CertificateBody,
    /// `certificate`: signer certificate digest.
    Certificate,
    /// `certificate-set`: count-prefixed ordered certificate digests (design C3).
    CertificateSet,
    /// `credential-body`: issuer-signed credential transcript (§2.4).
    CredentialBody,
    /// `credential`: credential digest.
    Credential,
    /// `scheme-policy-body`: signed scheme policy transcript (§6.1).
    SchemePolicyBody,
    /// `scheme-policy`: scheme policy digest.
    SchemePolicy,
    /// `fee-schedule-body`: signed fee schedule transcript (§6.2).
    FeeScheduleBody,
    /// `fee-schedule`: fee schedule digest.
    FeeSchedule,
    /// `blacklist-body`: signed blacklist transcript (§6.3).
    BlacklistBody,
    /// `blacklist`: blacklist digest.
    Blacklist,
    /// `blacklist-leaf`: blacklist gap leaf.
    BlacklistLeaf,
    /// `blacklist-node`: blacklist tree node.
    BlacklistNode,
    /// `quota-share-body`: signed quota share transcript (§6.4).
    QuotaShareBody,
    /// `quota-share`: quota share digest.
    QuotaShare,
    /// `quota-window`: quota window leaf.
    QuotaWindow,
    /// `quota-node`: quota window tree node.
    QuotaNode,
    /// `time-anchor-body`: signed time anchor transcript (§6.5).
    TimeAnchorBody,
    /// `time-anchor`: time anchor digest.
    TimeAnchor,
    /// `offer-body`: payer-signed Offer transcript (§4.1).
    OfferBody,
    /// `session-control-body`: session control transcript (§4.5).
    SessionControlBody,
    /// `request-body`: receiver-signed Request transcript (§4.2).
    RequestBody,
    /// `request`: Request digest.
    Request,
    /// `credit`: credit identity over the Request body transcript (§4.2).
    Credit,
    /// `dependencies`: positional Send verification dependencies (§4.3).
    Dependencies,
    /// `statement`: transition statement (§3.3).
    Statement,
    /// `proof`: transition or `CreditStatus` proof bytes (§3.4).
    Proof,
    /// `receipt-body`: provider commit receipt transcript (§3.5).
    ReceiptBody,
    /// `receipt`: provider commit receipt digest.
    Receipt,
    /// `package`: complete state package (§3.5).
    Package,
    /// `payment`: complete canonical Payment (§4.3).
    Payment,
    /// `credit-status-statement`: read-only `CreditStatus` statement (§4.4).
    CreditStatusStatement,
    /// `credited`: delivery evidence (§4.4).
    Credited,
    /// `operation-id`: provider operation identity (§3.6).
    OperationId,
    /// `output`: receipt-free output descriptor (§5.2).
    Output,
    /// `capsule`: local recovery capsule frame (§5.2).
    Capsule,
    /// `marker`: local provider marker frame (§5.1).
    Marker,
    /// `completion`: local completion record frame (§5.2).
    Completion,
    /// `voucher-body`: signed load voucher transcript (§7.1).
    VoucherBody,
    /// `voucher`: load voucher digest.
    Voucher,
    /// `unload-nullifier`: unload claim nullifier (§7.2).
    UnloadNullifier,
    /// `ledger-control-body`: wallet-key ledger control transcript (§7.3).
    LedgerControlBody,
    /// `renewal-challenge`: payment-key possession transcript (§2.5).
    RenewalChallenge,
    /// `renewal-key-binding`: payment-key binding of a newly attested key (§2.5).
    RenewalKeyBinding,
    /// `renewal-assertion`: App Attest renewal assertion client data (design C3).
    RenewalAssertion,
    /// `artifact-manifest-body`: signed artifact manifest transcript (design C4).
    ArtifactManifestBody,
    /// `artifact-manifest`: artifact manifest digest.
    ArtifactManifest,
    /// `charge-quote-body`: signed load/unload charge quote transcript (design C7).
    ChargeQuoteBody,
    /// `charge-quote`: charge quote digest.
    ChargeQuote,
    /// `evidence`: original platform evidence bytes (design C3).
    Evidence,
}

impl KagemushaWalletDigestRoleV1 {
    /// Every role, in declaration order.
    pub const ALL: [Self; 59] = [
        Self::Scheme,
        Self::Relation,
        Self::ProviderContract,
        Self::AssetScope,
        Self::Account,
        Self::EnrollmentChallenge,
        Self::EnrollmentId,
        Self::EnrollmentKeyBinding,
        Self::WalletId,
        Self::CertificateBody,
        Self::Certificate,
        Self::CertificateSet,
        Self::CredentialBody,
        Self::Credential,
        Self::SchemePolicyBody,
        Self::SchemePolicy,
        Self::FeeScheduleBody,
        Self::FeeSchedule,
        Self::BlacklistBody,
        Self::Blacklist,
        Self::BlacklistLeaf,
        Self::BlacklistNode,
        Self::QuotaShareBody,
        Self::QuotaShare,
        Self::QuotaWindow,
        Self::QuotaNode,
        Self::TimeAnchorBody,
        Self::TimeAnchor,
        Self::OfferBody,
        Self::SessionControlBody,
        Self::RequestBody,
        Self::Request,
        Self::Credit,
        Self::Dependencies,
        Self::Statement,
        Self::Proof,
        Self::ReceiptBody,
        Self::Receipt,
        Self::Package,
        Self::Payment,
        Self::CreditStatusStatement,
        Self::Credited,
        Self::OperationId,
        Self::Output,
        Self::Capsule,
        Self::Marker,
        Self::Completion,
        Self::VoucherBody,
        Self::Voucher,
        Self::UnloadNullifier,
        Self::LedgerControlBody,
        Self::RenewalChallenge,
        Self::RenewalKeyBinding,
        Self::RenewalAssertion,
        Self::ArtifactManifestBody,
        Self::ArtifactManifest,
        Self::ChargeQuoteBody,
        Self::ChargeQuote,
        Self::Evidence,
    ];

    /// Exact ASCII role label hashed after [`KAGEMUSHA_WALLET_DIGEST_PREFIX_V1`].
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheme => "scheme",
            Self::Relation => "relation",
            Self::ProviderContract => "provider-contract",
            Self::AssetScope => "asset-scope",
            Self::Account => "account",
            Self::EnrollmentChallenge => "enrollment-challenge",
            Self::EnrollmentId => "enrollment-id",
            Self::EnrollmentKeyBinding => "enrollment-key-binding",
            Self::WalletId => "wallet-id",
            Self::CertificateBody => "certificate-body",
            Self::Certificate => "certificate",
            Self::CertificateSet => "certificate-set",
            Self::CredentialBody => "credential-body",
            Self::Credential => "credential",
            Self::SchemePolicyBody => "scheme-policy-body",
            Self::SchemePolicy => "scheme-policy",
            Self::FeeScheduleBody => "fee-schedule-body",
            Self::FeeSchedule => "fee-schedule",
            Self::BlacklistBody => "blacklist-body",
            Self::Blacklist => "blacklist",
            Self::BlacklistLeaf => "blacklist-leaf",
            Self::BlacklistNode => "blacklist-node",
            Self::QuotaShareBody => "quota-share-body",
            Self::QuotaShare => "quota-share",
            Self::QuotaWindow => "quota-window",
            Self::QuotaNode => "quota-node",
            Self::TimeAnchorBody => "time-anchor-body",
            Self::TimeAnchor => "time-anchor",
            Self::OfferBody => "offer-body",
            Self::SessionControlBody => "session-control-body",
            Self::RequestBody => "request-body",
            Self::Request => "request",
            Self::Credit => "credit",
            Self::Dependencies => "dependencies",
            Self::Statement => "statement",
            Self::Proof => "proof",
            Self::ReceiptBody => "receipt-body",
            Self::Receipt => "receipt",
            Self::Package => "package",
            Self::Payment => "payment",
            Self::CreditStatusStatement => "credit-status-statement",
            Self::Credited => "credited",
            Self::OperationId => "operation-id",
            Self::Output => "output",
            Self::Capsule => "capsule",
            Self::Marker => "marker",
            Self::Completion => "completion",
            Self::VoucherBody => "voucher-body",
            Self::Voucher => "voucher",
            Self::UnloadNullifier => "unload-nullifier",
            Self::LedgerControlBody => "ledger-control-body",
            Self::RenewalChallenge => "renewal-challenge",
            Self::RenewalKeyBinding => "renewal-key-binding",
            Self::RenewalAssertion => "renewal-assertion",
            Self::ArtifactManifestBody => "artifact-manifest-body",
            Self::ArtifactManifest => "artifact-manifest",
            Self::ChargeQuoteBody => "charge-quote-body",
            Self::ChargeQuote => "charge-quote",
            Self::Evidence => "evidence",
        }
    }
}

/// Exact SHA-256 preimage of `H(role, body)`; the ECDSA message of a signed body.
///
/// The layout is `prefix || role || 0x00 || LE64(len(body)) || body`.
#[must_use]
pub fn kagemusha_wallet_preimage_v1(role: KagemushaWalletDigestRoleV1, body: &[u8]) -> Vec<u8> {
    let label = role.as_str().as_bytes();
    let capacity = KAGEMUSHA_WALLET_DIGEST_PREFIX_V1
        .len()
        .saturating_add(label.len())
        .saturating_add(9)
        .saturating_add(body.len());
    let mut preimage = Vec::with_capacity(capacity);
    preimage.extend_from_slice(KAGEMUSHA_WALLET_DIGEST_PREFIX_V1);
    preimage.extend_from_slice(label);
    preimage.push(0);
    preimage.extend_from_slice(&body_length_le64(body));
    preimage.extend_from_slice(body);
    preimage
}

/// Domain-separated digest `H(role, body)` (§8).
#[must_use]
pub fn kagemusha_wallet_digest_v1(role: KagemushaWalletDigestRoleV1, body: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_WALLET_DIGEST_PREFIX_V1);
    hasher.update(role.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(body_length_le64(body));
    hasher.update(body);
    hasher.finalize().into()
}

/// Digest of one signed object: `H(role, e || signature)`, where `e` is its body digest.
#[must_use]
pub fn kagemusha_wallet_signed_object_digest_v1(
    role: KagemushaWalletDigestRoleV1,
    body_digest: &[u8; 32],
    signature: &KagemushaDeviceSignatureV1,
) -> [u8; 32] {
    let body =
        WalletTranscriptV1::with_capacity(KAGEMUSHA_WALLET_SIGNED_OBJECT_TRANSCRIPT_BYTES_V1)
            .digest(body_digest)
            .signature(signature)
            .finish();
    kagemusha_wallet_digest_v1(role, &body)
}

/// Raw output of a platform P-256 signer, before it is frozen into a canonical object.
///
/// The variant is explicit: there is no length-based dispatch between encodings.
// The fixed 64-byte raw form stays inline; equalizing the variants would only add an
// allocation or an indirection to a short-lived signer handoff value.
#[allow(variant_size_differences)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaWalletSignerOutputV1<'a> {
    /// Strict DER, as returned by Android Keystore `SHA256withECDSA` or `CryptoKit`
    /// `derRepresentation`.
    Der(&'a [u8]),
    /// Fixed-width big-endian `r || s`, as returned by `CryptoKit` `rawRepresentation`.
    Raw([u8; 64]),
}

/// Normalize a fresh signer output to low S and verify it over `H(role, body)` under `key`.
///
/// Every constructor that embeds a fresh signature goes through this function, so a frozen
/// object never carries a high-S, malformed or non-verifying signature. Verifiers use
/// [`kagemusha_wallet_verify_signature_v1`], which rejects high S instead of rewriting it.
///
/// # Errors
///
/// Returns [`KagemushaWalletValidationErrorV1::InvalidSignature`] for non-canonical DER,
/// out-of-range scalars, or a signature that does not verify.
pub fn kagemusha_wallet_freeze_signature_v1(
    key: &KagemushaDevicePublicKeyV1,
    role: KagemushaWalletDigestRoleV1,
    body: &[u8],
    signer_output: KagemushaWalletSignerOutputV1<'_>,
) -> WalletResult<KagemushaDeviceSignatureV1> {
    let rejected = || KagemushaWalletValidationErrorV1::InvalidSignature { role };
    let signature = match signer_output {
        KagemushaWalletSignerOutputV1::Der(der) => {
            KagemushaDeviceSignatureV1::from_der_normalizing_low_s(der).map_err(|_| rejected())?
        }
        KagemushaWalletSignerOutputV1::Raw(raw) => {
            let parsed = P256Signature::from_slice(&raw).map_err(|_| rejected())?;
            let low_s = parsed.normalize_s().unwrap_or(parsed);
            KagemushaDeviceSignatureV1::from_raw_bytes(low_s.to_bytes().as_slice())
                .map_err(|_| rejected())?
        }
    };
    kagemusha_wallet_verify_signature_v1(key, role, body, &signature)?;
    Ok(signature)
}

/// Verify a received low-S signature over the exact preimage of `H(role, body)` under `key`.
///
/// # Errors
///
/// Returns [`KagemushaWalletValidationErrorV1::InvalidSignature`] when the key or signature
/// is not canonical or the signature does not verify.
pub fn kagemusha_wallet_verify_signature_v1(
    key: &KagemushaDevicePublicKeyV1,
    role: KagemushaWalletDigestRoleV1,
    body: &[u8],
    signature: &KagemushaDeviceSignatureV1,
) -> WalletResult<()> {
    signature
        .verify(key, &kagemusha_wallet_preimage_v1(role, body))
        .map_err(|_| KagemushaWalletValidationErrorV1::InvalidSignature { role })
}

fn body_length_le64(body: &[u8]) -> [u8; 8] {
    // `usize` is at most 64 bits on every admitted target, so the conversion is exact.
    u64::try_from(body.len()).unwrap_or(u64::MAX).to_le_bytes()
}

/// Fixed-layout transcript builder shared by the wallet object owners (design §1).
#[derive(Debug)]
pub(super) struct WalletTranscriptV1 {
    bytes: Vec<u8>,
}

impl WalletTranscriptV1 {
    /// Start an empty transcript with the exact expected capacity.
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    /// Append one byte, used for enum tags.
    pub(super) fn u8(mut self, value: u8) -> Self {
        self.bytes.push(value);
        self
    }

    /// Append a little-endian `u16`.
    pub(super) fn u16(mut self, value: u16) -> Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Append a little-endian `u32`.
    pub(super) fn u32(mut self, value: u32) -> Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Append a little-endian `u64`.
    pub(super) fn u64(mut self, value: u64) -> Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Append a little-endian `u128`.
    pub(super) fn u128(mut self, value: u128) -> Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Append `count` zero bytes, used to fill a fixed-width enum union.
    pub(super) fn zeros(mut self, count: usize) -> Self {
        self.bytes.resize(self.bytes.len().saturating_add(count), 0);
        self
    }

    /// Append one raw 32-byte digest or nonce.
    pub(super) fn digest(mut self, value: &[u8; 32]) -> Self {
        self.bytes.extend_from_slice(value);
        self
    }

    /// Append one 65-byte uncompressed SEC1 P-256 key.
    pub(super) fn key(self, key: &KagemushaDevicePublicKeyV1) -> Self {
        self.bytes(key.as_sec1_bytes())
    }

    /// Append one 64-byte `r || s` signature.
    pub(super) fn signature(self, signature: &KagemushaDeviceSignatureV1) -> Self {
        self.bytes(signature.as_raw_bytes())
    }

    /// Append raw bytes whose length is fixed by the caller's layout.
    pub(super) fn bytes(mut self, value: &[u8]) -> Self {
        self.bytes.extend_from_slice(value);
        self
    }

    /// Return the completed transcript.
    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
