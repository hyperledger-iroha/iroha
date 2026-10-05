//! Domain-separated digests, signing preimages and signature freezing (§8, design §1).

use p256::ecdsa::Signature as P256Signature;
use sha2::{Digest as _, Sha256};

use super::{
    KagemushaWalletValidationErrorV1, WalletResult,
    keys::{KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1},
};

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
/// suffix names that signed object's digest `H(role, e || signature)`. Every value a step
/// relation computes or opens is a Poseidon value of the σ field instead (§3): `credit_id`, map
/// leaves and roots, chains, the state commitment, the blacklist and quota-window trees, the
/// credit-digest tree, and the large-input digests `proof_digest` and the Payment digest
/// (`P_bytes`, [`super::poseidon`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KagemushaWalletDigestRoleV1 {
    /// `scheme`: scheme identity (§§2.2, 3.3).
    Scheme,
    /// `relation`: frozen relation identity (§§3.2, 3.3).
    Relation,
    /// `provider-contract`: Advance contract and receipt format identity (§2.2).
    ProviderContract,
    /// `asset-scope`: asset incarnation and scale (§2.2).
    AssetScope,
    /// `account`: canonical domainless `AccountId` frame.
    Account,
    /// `enrollment-challenge`: issuer enrollment challenge (§2.2).
    EnrollmentChallenge,
    /// `enrollment-id`: enrollment incarnation identity (§2.2).
    EnrollmentId,
    /// `enrollment-key-binding`: App Attest enrollment assertion client data (§2.2).
    EnrollmentKeyBinding,
    /// `wallet-id`: wallet incarnation identity (§2.2).
    WalletId,
    /// `certificate-body`: scheme-root-signed signer certificate transcript (§§2.3, 3.3).
    CertificateBody,
    /// `certificate`: signer certificate digest.
    Certificate,
    /// `certificate-set`: count-prefixed ordered certificate digests (design C3).
    CertificateSet,
    /// `credential-body`: issuer-signed credential transcript (§2.2).
    CredentialBody,
    /// `credential`: credential digest.
    Credential,
    /// `scheme-policy-body`: signed scheme policy transcript (§7).
    SchemePolicyBody,
    /// `scheme-policy`: scheme policy digest.
    SchemePolicy,
    /// `fee-schedule-body`: signed fee schedule transcript (§6.2).
    FeeScheduleBody,
    /// `fee-schedule`: fee schedule digest.
    FeeSchedule,
    /// `blacklist-body`: signed blacklist transcript (§7).
    BlacklistBody,
    /// `blacklist`: blacklist digest.
    Blacklist,
    /// `quota-share-body`: signed quota share transcript (§7).
    QuotaShareBody,
    /// `quota-share`: quota share digest.
    QuotaShare,
    /// `time-anchor-body`: signed time anchor transcript (§7).
    TimeAnchorBody,
    /// `time-anchor`: time anchor digest.
    TimeAnchor,
    /// `offer-body`: payer-signed Offer transcript (§5.1).
    OfferBody,
    /// `session-control-body`: session control transcript (§§5.1, 8).
    SessionControlBody,
    /// `request-body`: receiver-signed Request transcript (§5.1).
    RequestBody,
    /// `request`: Request digest.
    Request,
    /// `statement`: transition statement (§3.1).
    Statement,
    /// `lineage`: exact bytes of one lineage proof Ω with its public outputs (§3.2).
    Lineage,
    /// `receipt-body`: provider commit receipt transcript (§4.1).
    ReceiptBody,
    /// `receipt`: provider commit receipt digest.
    Receipt,
    /// `package`: complete state package (§3.1).
    Package,
    /// `credit-opening`: compressed credit-digest opening carried by `CreditStatus` (§5.1).
    CreditOpening,
    /// `credit-status`: read-only `CreditStatus` of a folded head (§§3.1, 5.1).
    CreditStatus,
    /// `credited`: delivery evidence (§5.1).
    Credited,
    /// `operation-id`: provider operation identity (§4.1).
    OperationId,
    /// `output`: receipt-free output descriptor (§4.1).
    Output,
    /// `capsule`: local recovery capsule frame (§4.1).
    Capsule,
    /// `marker`: local provider marker frame (§4.2).
    Marker,
    /// `completion`: local completion record frame (§4.1).
    Completion,
    /// `fold`: durable fold record of one self-verified Ω (§3.1 step 5).
    Fold,
    /// `voucher-body`: signed load voucher transcript (§6.1).
    VoucherBody,
    /// `voucher`: load voucher digest.
    Voucher,
    /// `unload-nullifier`: unload claim nullifier (§6.1).
    UnloadNullifier,
    /// `ledger-control-body`: wallet-key ledger control transcript (§§3.2, 6.3).
    LedgerControlBody,
    /// `renewal-challenge`: payment-key possession transcript (§2.2).
    RenewalChallenge,
    /// `renewal-key-binding`: payment-key binding of a newly attested key (§2.2).
    RenewalKeyBinding,
    /// `renewal-assertion`: App Attest renewal assertion client data (design C3).
    RenewalAssertion,
    /// `artifact-manifest-body`: signed artifact manifest transcript (design C4).
    ArtifactManifestBody,
    /// `artifact-manifest`: artifact manifest digest.
    ArtifactManifest,
    /// `verifying-key-set`: the σ verifying-key allowlist whose digest is the manifest's
    /// `verifying_key_set_digest` (§3.2, owner answer Q11).
    VerifyingKeySet,
    /// `charge-quote-body`: signed load/unload charge quote transcript (design C7).
    ChargeQuoteBody,
    /// `charge-quote`: charge quote digest.
    ChargeQuote,
    /// `evidence`: original platform evidence bytes (design C3).
    Evidence,
}

impl KagemushaWalletDigestRoleV1 {
    /// Every role, in declaration order.
    pub const ALL: [Self; 55] = [
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
        Self::QuotaShareBody,
        Self::QuotaShare,
        Self::TimeAnchorBody,
        Self::TimeAnchor,
        Self::OfferBody,
        Self::SessionControlBody,
        Self::RequestBody,
        Self::Request,
        Self::Statement,
        Self::Lineage,
        Self::ReceiptBody,
        Self::Receipt,
        Self::Package,
        Self::CreditOpening,
        Self::CreditStatus,
        Self::Credited,
        Self::OperationId,
        Self::Output,
        Self::Capsule,
        Self::Marker,
        Self::Completion,
        Self::Fold,
        Self::VoucherBody,
        Self::Voucher,
        Self::UnloadNullifier,
        Self::LedgerControlBody,
        Self::RenewalChallenge,
        Self::RenewalKeyBinding,
        Self::RenewalAssertion,
        Self::ArtifactManifestBody,
        Self::ArtifactManifest,
        Self::VerifyingKeySet,
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
            Self::QuotaShareBody => "quota-share-body",
            Self::QuotaShare => "quota-share",
            Self::TimeAnchorBody => "time-anchor-body",
            Self::TimeAnchor => "time-anchor",
            Self::OfferBody => "offer-body",
            Self::SessionControlBody => "session-control-body",
            Self::RequestBody => "request-body",
            Self::Request => "request",
            Self::Statement => "statement",
            Self::Lineage => "lineage",
            Self::ReceiptBody => "receipt-body",
            Self::Receipt => "receipt",
            Self::Package => "package",
            Self::CreditOpening => "credit-opening",
            Self::CreditStatus => "credit-status",
            Self::Credited => "credited",
            Self::OperationId => "operation-id",
            Self::Output => "output",
            Self::Capsule => "capsule",
            Self::Marker => "marker",
            Self::Completion => "completion",
            Self::Fold => "fold",
            Self::VoucherBody => "voucher-body",
            Self::Voucher => "voucher",
            Self::UnloadNullifier => "unload-nullifier",
            Self::LedgerControlBody => "ledger-control-body",
            Self::RenewalChallenge => "renewal-challenge",
            Self::RenewalKeyBinding => "renewal-key-binding",
            Self::RenewalAssertion => "renewal-assertion",
            Self::ArtifactManifestBody => "artifact-manifest-body",
            Self::ArtifactManifest => "artifact-manifest",
            Self::VerifyingKeySet => "verifying-key-set",
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

// ---------------------------------------------------------------------------------------
// σ-field encodings (§§3, 3.2, 8; design §1.2 and §1.3)
// ---------------------------------------------------------------------------------------

/// Canonical little-endian encoding of the σ-field modulus
/// `p = 0x40000000000000000000000000000000224698fc094cf91b992d30ed00000001`.
///
/// The step relations σ are single-parity on Vesta `Eq` (§3.2), so every value they compute
/// or open (state commitment, chains, map roots, the statement digest) is an element of the
/// Vesta scalar field, Pasta `Fp`. A field value travels as its 32-byte little-endian
/// encoding, which must be `< p`; a noncanonical encoding is rejected before mutation (§8).
pub const KAGEMUSHA_WALLET_FIELD_MODULUS_V1: [u8; 32] = [
    0x01, 0x00, 0x00, 0x00, 0xed, 0x30, 0x2d, 0x99, 0x1b, 0xf9, 0x4c, 0x09, 0xfc, 0x98, 0x46, 0x22,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40,
];

/// Whether `value` is a canonical little-endian σ-field encoding, `value < p` (design §1.2).
///
/// This is a pure byte comparison against [`KAGEMUSHA_WALLET_FIELD_MODULUS_V1`], from the
/// most significant byte down; no field arithmetic is involved.
#[must_use]
pub fn kagemusha_wallet_is_canonical_field_v1(value: &[u8; 32]) -> bool {
    for (byte, modulus) in value
        .iter()
        .rev()
        .zip(KAGEMUSHA_WALLET_FIELD_MODULUS_V1.iter().rev())
    {
        match byte.cmp(modulus) {
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Greater => return false,
            core::cmp::Ordering::Equal => {}
        }
    }
    false
}

/// Canonical σ-field encoding of one integer element: `value` little-endian in the low 16
/// bytes, zero in the high 16 bytes (design §1.3). Every `u128` is below `p`.
#[must_use]
pub fn kagemusha_wallet_field_from_u128_v1(value: u128) -> [u8; 32] {
    let mut encoding = [0_u8; 32];
    encoding[..16].copy_from_slice(&value.to_le_bytes());
    encoding
}

/// Builder of one ordered σ-field element list (design §1.3 limb rule).
///
/// An integer (`u8` to `u128`, an enum tag or a mask) is one element; a 32-byte SHA-256
/// digest or identifier is two elements, its little-endian 128-bit limbs, low half first; a
/// field value (commitment, chain, Poseidon root, nonce) is one element, its canonical
/// encoding. The callers validate field values before they are appended.
#[derive(Debug)]
pub(super) struct WalletFieldItemsV1 {
    items: Vec<[u8; 32]>,
}

impl WalletFieldItemsV1 {
    /// Start an empty list with the exact expected capacity.
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    /// Append one integer element.
    pub(super) fn integer(mut self, value: u128) -> Self {
        self.items.push(kagemusha_wallet_field_from_u128_v1(value));
        self
    }

    /// Append a 32-byte digest as its two little-endian 128-bit limbs, low half first.
    pub(super) fn digest(self, value: &[u8; 32]) -> Self {
        let mut low = [0_u8; 16];
        let mut high = [0_u8; 16];
        low.copy_from_slice(&value[..16]);
        high.copy_from_slice(&value[16..]);
        self.integer(u128::from_le_bytes(low))
            .integer(u128::from_le_bytes(high))
    }

    /// Append one canonical field value.
    pub(super) fn field(mut self, value: &[u8; 32]) -> Self {
        self.items.push(*value);
        self
    }

    /// Append `count` zero elements, used to fill a fixed-width effect union.
    pub(super) fn zeros(mut self, count: usize) -> Self {
        self.items
            .resize(self.items.len().saturating_add(count), [0; 32]);
        self
    }

    /// Number of elements appended so far.
    pub(super) fn len(&self) -> usize {
        self.items.len()
    }

    /// Return the completed element list.
    pub(super) fn finish(self) -> Vec<[u8; 32]> {
        self.items
    }
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
