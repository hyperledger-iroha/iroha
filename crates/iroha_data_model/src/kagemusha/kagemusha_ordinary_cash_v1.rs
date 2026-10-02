//! First-release ordinary cash data, with separate Native custody and platform approval.
//!
//! These records describe financial selections; decoding, shape validation, SHA identifiers
//! and even a receiver signature do not create a financial loan or authorize publication.
//! Native must retain the genuine release, FI, both parties' issuer-authenticated credentials,
//! exact current signed observation originals, current Integrity leases and the original
//! purpose2/purpose1 approval WAL. Captured zero-Bootstrap approval is never a cash input.
//!
//! Preparation, output and terminal transcripts have an explicit acyclic order. Pre-approval
//! transition scope excludes approval evidence; the later prepared ID binds exact purpose2
//! authorization. The terminal body precedes purpose1 approval and excludes that approval
//! and the final output binding. The later logical record binds both authorizations.

#[path = "kagemusha_ordinary_cash_v1/ordinary_redeem.rs"]
mod ordinary_redeem;
pub use ordinary_redeem::*;

use super::{
    KAGEMUSHA_ASSET_SCALE_MAX_V1, KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1,
    KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1, KAGEMUSHA_REQUEST_MAX_TTL_MS_V1,
    KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1, KagemushaAppAttestReleaseMeasurementV1,
    KagemushaAppOperationApprovalEvidenceV1, KagemushaHardwarePlatformClassV1,
    KagemushaVerifiedOrdinaryAppCredentialV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use core::ops::Range;
use iroha_crypto::kex::{KeyExchangeScheme as _, X25519Sha256};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Maximum complete canonical ordinary receiver request, including its platform original.
pub const KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_MAX_BYTES_V1: usize = 4096;
/// Maximum complete canonical ordinary outgoing data record or logical terminal record.
pub const KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1: usize = 4096;
/// Exact request signing domain, including NUL.
pub const KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-payment-request\0";
/// Exact ordinary pre-candidate send output domain, including NUL.
pub const KAGEMUSHA_ORDINARY_PAYMENT_OUTPUT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-send-output\0";
/// Exact ordinary pre-approval transition domain, including NUL.
pub const KAGEMUSHA_ORDINARY_PREPARED_TRANSITION_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-prepared-transition\0";
/// Exact later ordinary preparation identifier domain, including NUL.
pub const KAGEMUSHA_ORDINARY_PREPARED_OUTGOING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-outgoing-preparation\0";
/// Exact interval clock-context binding domain, including NUL.
pub const KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-cash-clock\0";
/// Exact pre-approval terminal intent binding domain, including NUL.
pub const KAGEMUSHA_ORDINARY_CASH_TERMINAL_INTENT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-cash-terminal-intent\0";
/// Exact pre-approval terminal body binding domain, including NUL.
pub const KAGEMUSHA_ORDINARY_CASH_TERMINAL_BODY_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-cash-terminal-body\0";
/// Exact post-approval logical terminal-record binding domain, including NUL.
pub const KAGEMUSHA_ORDINARY_CASH_TERMINAL_RECORD_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-cash-terminal-record\0";
/// Ordinary request-bound credit identity domain, including NUL.
pub const KAGEMUSHA_ORDINARY_CREDIT_ID_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-credit-id\0";
/// Pre-candidate transport semantic domain, including NUL.
pub const KAGEMUSHA_ORDINARY_PAYMENT_BODY_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-payment-body\0";
/// Final post-approval output binding domain, including NUL.
pub const KAGEMUSHA_ORDINARY_OUTPUT_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-output-binding\0";
/// Ordinary normalized terminal Guard's pre-W1 commit-binding domain, including NUL.
pub const KAGEMUSHA_ORDINARY_TERMINAL_GUARD_COMMIT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-terminal-guard-commit\0";
/// Ordinary predecessor financial conflict domain, including NUL.
pub const KAGEMUSHA_ORDINARY_TRANSITION_NULLIFIER_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-terminal-conflict\0";
/// Maintained transition-stream commitment domain, including the original NUL delimiter.
/// Shared with the State carrier relation; no ordinary/OEM authority is conferred by this SHA.
pub const KAGEMUSHA_ORDINARY_SEALED_TRANSITION_INPUTS_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:sealed-transition-inputs\0";
/// Maintained recovery-stream commitment domain, including the original NUL delimiter.
pub const KAGEMUSHA_ORDINARY_SEALED_RECOVERY_SEEDS_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:sealed-recovery-seeds\0";

/// Commit every actual sealed-transition byte under the maintained State carrier grammar.
/// The exact preimage is the domain including NUL, LE64(actual byte length), then all raw bytes.
/// This is data only; it creates no Native custody, approval, financial witness or proof.
/// # Errors
/// Rejects empty streams and lengths beyond the actual released fixed maximum.
pub fn kagemusha_ordinary_sealed_transition_inputs_digest_v1(
    bytes: &[u8],
) -> Result<[u8; 32], String> {
    sealed_stream_digest(
        KAGEMUSHA_ORDINARY_SEALED_TRANSITION_INPUTS_DOMAIN_V1,
        KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
        bytes,
    )
}
/// Commit every actual sealed-recovery byte under the maintained State carrier grammar.
/// The exact preimage is the domain including NUL, LE64(actual byte length), then all raw bytes.
/// This is data only and is independent of a preparation or terminal approval grant.
/// # Errors
/// Rejects empty streams and lengths beyond the actual released fixed maximum.
pub fn kagemusha_ordinary_sealed_recovery_seeds_digest_v1(
    bytes: &[u8],
) -> Result<[u8; 32], String> {
    sealed_stream_digest(
        KAGEMUSHA_ORDINARY_SEALED_RECOVERY_SEEDS_DOMAIN_V1,
        KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1,
        bytes,
    )
}
fn sealed_stream_digest(domain: &[u8], maximum: u32, bytes: &[u8]) -> Result<[u8; 32], String> {
    let length = u64::try_from(bytes.len()).map_err(|_| "sealed stream length exceeds u64")?;
    if length == 0 || length > u64::from(maximum) {
        return Err("sealed stream length lies outside the actual fixed profile".into());
    }
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(length.to_le_bytes());
    hash.update(bytes);
    Ok(hash.finalize().into())
}

const ORIGINAL_DOMAIN: &[u8] = b"iroha:kagemusha:v1:ordinary-cash-original\0";

/// One model-owned raw field range in a fixed mathematical transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaOrdinaryCashTranscriptFieldV1 {
    /// Stable semantic field name; nested fields use a dot-separated prefix.
    pub name: &'static str,
    /// Absolute raw byte range in `KagemushaOrdinaryCashTranscriptV1::bytes`.
    pub range: Range<usize>,
}
/// Exact mathematical preimage and its encoder-owned field ranges.
/// This is distinct from the complete canonical Norito original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaOrdinaryCashTranscriptV1 {
    /// Entire domain-separated mathematical message.
    pub bytes: Vec<u8>,
    /// Complete ordered semantic raw-field ranges, excluding domain and fixed length prefix.
    pub fields: Vec<KagemushaOrdinaryCashTranscriptFieldV1>,
}
impl KagemushaOrdinaryCashTranscriptV1 {
    /// Return the SHA-256 of the complete model-owned mathematical message.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(&self.bytes).into()
    }
    /// Borrow one encoder-owned raw field range by semantic name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<Range<usize>> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.range.clone())
    }
}
/// Raw semantic positions in the sole complete canonical Norito original.
/// All schema, enum and sequence framing is pinned; only listed raw positions and CRC vary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaOrdinaryCashOriginalLayoutV1 {
    /// Domain + LE64(original length) + exact canonical-frame template.
    pub bytes: Vec<Option<u8>>,
    /// Complete canonical frame range within `bytes`.
    pub original: Range<usize>,
    /// Offset of the bare canonical payload within the original frame.
    pub payload_offset: usize,
    /// Ordered raw semantic byte positions, independently discovered from the sole encoder.
    pub fields: Vec<KagemushaOrdinaryCashOriginalFieldV1>,
}
/// Raw field positions in an exact original preimage; integer bytes are little-endian.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaOrdinaryCashOriginalFieldV1 {
    /// Stable semantic field name, matching the mathematical transcript where applicable.
    pub name: &'static str,
    /// Absolute raw semantic byte positions in `KagemushaOrdinaryCashOriginalLayoutV1::bytes`.
    pub positions: Vec<usize>,
}

struct Transcript(KagemushaOrdinaryCashTranscriptV1);
impl Transcript {
    fn new(domain: &[u8]) -> Self {
        Self(KagemushaOrdinaryCashTranscriptV1 {
            bytes: domain.to_vec(),
            fields: Vec::new(),
        })
    }
    fn raw(&mut self, name: &'static str, bytes: &[u8]) {
        let start = self.0.bytes.len();
        self.0.bytes.extend_from_slice(bytes);
        self.0.fields.push(KagemushaOrdinaryCashTranscriptFieldV1 {
            name,
            range: start..self.0.bytes.len(),
        });
    }
    fn embedded(&mut self, prefix: &'static str, value: &KagemushaOrdinaryCashTranscriptV1) {
        // Embedded records carry their fixed payload, not a second domain or length prefix.
        let start = self.0.bytes.len();
        self.0.bytes.extend_from_slice(&value.bytes);
        self.0.fields.push(KagemushaOrdinaryCashTranscriptFieldV1 {
            name: prefix,
            range: start..self.0.bytes.len(),
        });
    }
    fn finish(self) -> KagemushaOrdinaryCashTranscriptV1 {
        self.0
    }
}
fn nonzero(fields: &[[u8; 32]]) -> Result<(), String> {
    if fields.contains(&[0; 32]) {
        return Err("ordinary cash selector absent".into());
    }
    Ok(())
}
fn version(version: u16) -> Result<(), String> {
    if version != 1 {
        return Err("ordinary cash version differs".into());
    }
    Ok(())
}
fn outgoing(operation: u8) -> Result<(), String> {
    if !matches!(operation, 2 | 4) {
        return Err("ordinary cash operation is not SendSplit/RedeemSplit".into());
    }
    Ok(())
}
fn operation_slot(operation: u8, value: [u8; 32], send: bool) -> Result<(), String> {
    let required = if send { operation == 2 } else { operation == 4 };
    if (value != [0; 32]) != required {
        return Err("ordinary cash purpose-specific slot differs".into());
    }
    Ok(())
}
fn stream_bounds(lengths: [u64; 2], digests: [[u8; 32]; 2]) -> Result<(), String> {
    nonzero(&digests)?;
    for (length, max) in lengths.into_iter().zip([
        KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
        KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1,
    ]) {
        if length == 0 || length > u64::from(max) {
            return Err("ordinary sealed stream length differs".into());
        }
    }
    Ok(())
}
fn index_step(
    before: u128,
    after: u128,
    sequence_before: u64,
    sequence_after: u64,
) -> Result<(), String> {
    if before.checked_add(1) != Some(after)
        || sequence_before.checked_add(1) != Some(sequence_after)
    {
        return Err("ordinary cash independent index/sequence step differs".into());
    }
    Ok(())
}
fn bounded_encode<T: norito::NoritoSerialize>(value: &T, max: usize) -> Result<Vec<u8>, String> {
    let bytes = norito::encode_canonical(value).map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > max {
        return Err("ordinary cash canonical archive bound differs".into());
    }
    Ok(bytes)
}
fn exact_decode<T>(bytes: &[u8], max: usize) -> Result<T, String>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > max {
        return Err("ordinary cash original archive bound differs".into());
    }
    let value: T =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|e| e.to_string())?;
    if bounded_encode(&value, max)? != bytes {
        return Err("ordinary cash original is not canonical".into());
    }
    Ok(value)
}
fn original_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(ORIGINAL_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

struct OriginalBuilder<'a, T> {
    value: &'a T,
    frame: Vec<u8>,
    payload_offset: usize,
    layout: KagemushaOrdinaryCashOriginalLayoutV1,
}
impl<'a, T> OriginalBuilder<'a, T>
where
    T: Clone + norito::NoritoSerialize + norito::SerializePayload,
{
    fn new(value: &'a T, max: usize) -> Result<Self, String> {
        let frame = bounded_encode(value, max)?;
        let flags = *frame.get(39).ok_or("ordinary cash frame header absent")?;
        let payload =
            super::kagemusha_ordinary_app_enrollment_v1::layout_field_payload(value, flags)?;
        let payload_offset = frame
            .len()
            .checked_sub(payload.len())
            .filter(|offset| *offset >= norito::core::Header::SIZE)
            .ok_or("ordinary cash payload offset differs")?;
        if frame.get(payload_offset..) != Some(payload.as_slice())
            || frame[norito::core::Header::SIZE..payload_offset]
                .iter()
                .any(|b| *b != 0)
        {
            return Err("ordinary cash canonical root framing differs".into());
        }
        let mut bytes = ORIGINAL_DOMAIN
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>();
        bytes.extend((frame.len() as u64).to_le_bytes().into_iter().map(Some));
        let start = bytes.len();
        bytes.extend(frame.iter().copied().map(Some));
        bytes[start + 31..start + 39].fill(None);
        let end = bytes.len();
        Ok(Self {
            value,
            frame,
            payload_offset,
            layout: KagemushaOrdinaryCashOriginalLayoutV1 {
                bytes,
                original: start..end,
                payload_offset,
                fields: Vec::new(),
            },
        })
    }
    fn field(
        &mut self,
        name: &'static str,
        raw: &[u8],
        mutate: impl Fn(&mut T, usize),
    ) -> Result<(), String> {
        let mut positions = Vec::with_capacity(raw.len());
        for (index, byte) in raw.iter().copied().enumerate() {
            let mut changed = self.value.clone();
            mutate(&mut changed, index);
            let frame = norito::encode_canonical(&changed).map_err(|e| e.to_string())?;
            if frame.len() != self.frame.len()
                || frame[..31] != self.frame[..31]
                || frame[39..self.payload_offset] != self.frame[39..self.payload_offset]
            {
                return Err("ordinary cash semantic mutation changed framing".into());
            }
            let differences = (self.payload_offset..frame.len())
                .filter(|i| frame[*i] != self.frame[*i])
                .collect::<Vec<_>>();
            if differences.len() != 1
                || self.frame[differences[0]] != byte
                || frame[differences[0]] != (byte ^ 1)
            {
                return Err("ordinary cash semantic byte encoder differs".into());
            }
            let position = self.layout.original.start + differences[0];
            if self.layout.bytes[position].is_none() {
                return Err("ordinary cash original fields overlap".into());
            }
            self.layout.bytes[position] = None;
            positions.push(position);
        }
        self.layout
            .fields
            .push(KagemushaOrdinaryCashOriginalFieldV1 { name, positions });
        Ok(())
    }
    fn finish(self) -> KagemushaOrdinaryCashOriginalLayoutV1 {
        self.layout
    }
}

/// Pure request-bound ordinary credit identity; no receiver or transport admission.
#[must_use]
pub fn kagemusha_ordinary_credit_id_v1(nullifier: [u8; 32], request: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_CREDIT_ID_DOMAIN_V1);
    hash.update(nullifier);
    hash.update(request);
    hash.finalize().into()
}
/// Pre-candidate transport semantic SHA. Both inputs are opened independently by the consumer.
/// # Errors
/// Rejects an absent pre-candidate output or actual encrypted-byte digest.
pub fn kagemusha_ordinary_payment_body_digest_v1(
    output: [u8; 32],
    encrypted: [u8; 32],
) -> Result<[u8; 32], String> {
    nonzero(&[output, encrypted])?;
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_PAYMENT_BODY_DOMAIN_V1);
    hash.update(output);
    hash.update(encrypted);
    Ok(hash.finalize().into())
}
/// Final output binding formed only after the candidate and exact logical terminal record exist.
/// This data function neither verifies those inputs nor authorizes money.
/// # Errors
/// Rejects an absent prepared semantic, candidate or terminal-record digest.
pub fn kagemusha_ordinary_output_binding_digest_v1(
    projection: [u8; 32],
    candidate: [u8; 32],
    record: [u8; 32],
) -> Result<[u8; 32], String> {
    nonzero(&[projection, candidate, record])?;
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_OUTPUT_BINDING_DOMAIN_V1);
    hash.update(projection);
    hash.update(candidate);
    hash.update(record);
    Ok(hash.finalize().into())
}
/// Data-only first-release ordinary `CashClockContext` record.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryCashClockContextV1")]
pub struct KagemushaOrdinaryCashClockContextV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact nonce independently reserved before the four signed validator observations.
    pub request_nonce: [u8; 32],
    /// SHA256(domain iroha:kagemusha:v1:ordinary-native-clock-signed-observations\0, nonce32, `certified_context_id32`, then four installed-order `LE32(original_len)` + full canonical signed attestation originals). Excludes mutable WAL ceilings and interval bounds.
    pub signed_observations_original_digest: [u8; 32],
    /// Conservative inclusive lower Unix-ms bound retained by the actual Native clock owner.
    pub lower_at_ms: u64,
    /// Conservative inclusive upper Unix-ms bound retained by the same Native clock owner.
    pub upper_at_ms: u64,
}
/// Data-only first-release ordinary `PaymentRequestBody` record.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryPaymentRequestBodyV1")]
pub struct KagemushaOrdinaryPaymentRequestBodyV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Actual independently admitted release.
    pub release_id: [u8; 32],
    /// Actual network selector.
    pub network_id: [u8; 32],
    /// Model-normalized authoritative asset selector.
    pub normalized_asset_id: [u8; 32],
    /// Actual authoritative asset incarnation.
    pub asset_incarnation: [u8; 32],
    /// Actual governed asset scale.
    pub scale: u32,
    /// Actual common reserve pool.
    pub reserve_pool_id: [u8; 32],
    /// Receiver account binding in the same credential signing domain.
    pub recipient_account_binding: [u8; 32],
    /// Positive requested amount in the exact governed scale.
    pub amount: u128,
    /// Actual valid X25519 receiver key, independently retained before encryption.
    pub recipient_encryption_key: [u8; 32],
    /// Exact complete issuer-authenticated ordinary receiver credential original digest.
    pub recipient_credential_digest: [u8; 32],
    /// Actual receiver lane from that same credential.
    pub recipient_lane_id: [u8; 32],
    /// Native-reserved receiver request identity.
    pub request_id: [u8; 32],
    /// Actual original interval projection; its four signed originals remain in Native custody.
    pub clock_context: KagemushaOrdinaryCashClockContextV1,
    /// Inclusive original request issuance bound.
    pub issued_at_ms: u64,
    /// Exclusive original request expiry; never renewed by decoding or signing.
    pub expires_at_ms: u64,
}
/// Data-only first-release ordinary `PaymentOutput` record.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryPaymentOutputV1")]
pub struct KagemushaOrdinaryPaymentOutputV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact full canonical ordinary receiver request original digest.
    pub request_digest: [u8; 32],
    /// Actual positive amount selected from the State and receiver request.
    pub amount: u128,
    /// Actual predecessor hiding State head.
    pub sender_before_commitment: [u8; 32],
    /// Actual successor hiding State head.
    pub sender_after_commitment: [u8; 32],
    /// Proof-derived outgoing transition nullifier.
    pub transition_nullifier: [u8; 32],
    /// Ordinary request-bound credit identity derived from that nullifier.
    pub credit_id: [u8; 32],
    /// Actual amount-bound ciphertext semantic commitment.
    pub ciphertext_commitment: [u8; 32],
    /// Sole maintained ciphertext digest of the complete actual encrypted-credit bytes.
    pub encrypted_credit_digest: [u8; 32],
    /// Exact pre-candidate interval-context binding digest, selected by Native.
    pub clock_context_digest: [u8; 32],
    /// Conservative preparation upper bound; this is not a terminal commit time.
    pub prepared_at_ms: u64,
}
/// Data-only first-release ordinary `PreparedTransition` record.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryPreparedTransitionV1")]
pub struct KagemushaOrdinaryPreparedTransitionV1 {
    /// Sole first-release version.
    pub version: u16,
    /// `SendSplit2` or `RedeemSplit4`.
    pub operation: u8,
    /// Actual canonical lifecycle context digest.
    pub lifecycle_digest: [u8; 32],
    /// Exact ordinary request original digest for `SendSplit`; zero for `RedeemSplit`.
    pub request_digest: [u8; 32],
    /// Actual predecessor State head for either operation.
    pub predecessor_state: [u8; 32],
    /// Actual successor State head for either operation.
    pub successor_state: [u8; 32],
    /// Actual positive State-selected amount.
    pub amount: u128,
    /// Exact independently selected pre-candidate financial reservation.
    pub reservation_digest: [u8; 32],
    /// Actual purpose2 Native operation ID reserved before W; never SHA(W).
    pub native_preparation_operation_id: [u8; 32],
}
/// Data-only first-release ordinary `PreparedOutgoing` record.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryPreparedOutgoingV1")]
pub struct KagemushaOrdinaryPreparedOutgoingV1 {
    /// Sole first-release version.
    pub version: u16,
    /// `SendSplit2` or `RedeemSplit4`.
    pub operation: u8,
    /// Actual predecessor State head.
    pub predecessor_state: [u8; 32],
    /// Actual successor State head.
    pub successor_state: [u8; 32],
    /// Exact full transition statement SHA.
    pub transition_digest: [u8; 32],
    /// Exact acyclic pre-approval transition binding.
    pub prepared_transition_binding_digest: [u8; 32],
    /// Actual pre-candidate transport or redemption semantic digest.
    pub projection_semantic_digest: [u8; 32],
    /// Actual canonical lifecycle digest.
    pub lifecycle_binding_digest: [u8; 32],
    /// Exact ordinary request digest for `SendSplit`; zero for `RedeemSplit`.
    pub request_digest: [u8; 32],
    /// Actual release artifact manifest for `RedeemSplit`; zero for `SendSplit`.
    pub artifact_manifest_digest: [u8; 32],
    /// Exact normalized genuine purpose2 Guard digest.
    pub preparation_guard_digest: [u8; 32],
    /// Same actual pre-candidate reservation.
    pub reservation_digest: [u8; 32],
    /// Exact genuine purpose2 W plus selected complete Integrity lease third Guard column.
    pub preparation_authorization_digest: [u8; 32],
    /// Complete retained sealed transition and recovery stream lengths.
    #[norito(json = "crate::json_helpers::fixed_pair")]
    pub stream_lengths: [u64; 2],
    /// Complete retained sealed transition and recovery stream SHA digests.
    #[norito(json = "crate::json_helpers::fixed_pair")]
    pub stream_digests: [[u8; 32]; 2],
}
/// Data-only first-release ordinary `CashTerminalIntent` record.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryCashTerminalIntentV1")]
pub struct KagemushaOrdinaryCashTerminalIntentV1 {
    /// Sole first-release version.
    pub version: u16,
    /// `SendSplit2` or `RedeemSplit4`.
    pub operation: u8,
    /// Actual purpose1 Native operation ID, independently reserved before W1.
    pub native_operation_id: [u8; 32],
    /// Actual independently reserved original purpose1 nonce.
    pub native_nonce: [u8; 32],
    /// Exact late purpose2 preparation identifier.
    pub preparation_id: [u8; 32],
    /// Exact candidate State/proof protocol digest selected before terminal approval.
    pub candidate_digest: [u8; 32],
    /// Exact complete actual State transition statement SHA.
    pub state_statement_digest: [u8; 32],
    /// Exact current held financial State/proof descriptor prefix.
    pub predecessor_descriptor_prefix_digest: [u8; 32],
    /// Exact sender ordinary credential original selected by the held financial owner.
    pub sender_credential_digest: [u8; 32],
    /// Same actual financial reservation bound by purpose2 preparation.
    pub reservation_digest: [u8; 32],
    /// Actual financial State secure index, independent of logical journal sequence and Apple counter.
    pub secure_index_before: u128,
    /// Actual next financial State secure index.
    pub secure_index_after: u128,
    /// Actual held software financial journal revision; distinct from `State.logical_sequence:u128`.
    pub logical_journal_sequence_before: u64,
    /// Actual next software financial journal revision, with no financial sequence cast.
    pub logical_journal_sequence_after: u64,
    /// Original purpose1 approval interval lower bound.
    pub issued_at_ms: u64,
    /// Original exclusive purpose1 approval expiry; at most 120 seconds later.
    pub expires_at_ms: u64,
}
/// Data-only first-release ordinary `CashTerminalBody` record.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryCashTerminalBodyV1")]
pub struct KagemushaOrdinaryCashTerminalBodyV1 {
    /// Sole first-release version.
    pub version: u16,
    /// `SendSplit2` or `RedeemSplit4`.
    pub operation: u8,
    /// Actual positive amount joined to the genuine State candidate.
    pub amount: u128,
    /// Exact complete State statement SHA signed in the full selection subject.
    pub state_statement_digest: [u8; 32],
    /// Same actual selected candidate digest.
    pub candidate_digest: [u8; 32],
    /// Exact late purpose2 preparation identifier.
    pub preparation_id: [u8; 32],
    /// Actual pre-candidate semantic projection; never final output binding.
    pub prepared_projection_semantic_digest: [u8; 32],
    /// Same canonical lifecycle context.
    pub lifecycle_digest: [u8; 32],
    /// Exact full ordinary receiver request original for `SendSplit`; zero for `RedeemSplit`.
    pub request_digest: [u8; 32],
    /// Exact ordinary receiver credential original for `SendSplit`; zero for `RedeemSplit`.
    pub recipient_credential_digest: [u8; 32],
    /// Actual raw pre-candidate ordinary send output digest; zero for `RedeemSplit`.
    pub send_output_digest: [u8; 32],
    /// Complete actual encrypted-credit digest for `SendSplit`; zero for `RedeemSplit`.
    pub encrypted_credit_digest: [u8; 32],
    /// Actual release artifact manifest for `RedeemSplit`; zero for `SendSplit`.
    pub artifact_manifest_digest: [u8; 32],
    /// Same retained pre-candidate financial reservation.
    pub reservation_digest: [u8; 32],
    /// Same actual reserved purpose1 operation ID.
    pub native_operation_id: [u8; 32],
    /// Exact pre-approval terminal intent digest, containing the original nonce and sender C.
    pub terminal_intent_digest: [u8; 32],
    /// Same current held financial State/proof descriptor prefix.
    pub predecessor_descriptor_prefix_digest: [u8; 32],
    /// Exact sealed transition and recovery stream lengths from preparation.
    #[norito(json = "crate::json_helpers::fixed_pair")]
    pub stream_lengths: [u64; 2],
    /// Exact sealed transition and recovery stream SHA digests from preparation.
    #[norito(json = "crate::json_helpers::fixed_pair")]
    pub stream_digests: [[u8; 32]; 2],
    /// Actual before-W1 interval context selected by Native, with complete originals retained.
    pub clock_context: KagemushaOrdinaryCashClockContextV1,
    /// Actual predecessor financial State secure index.
    pub secure_index_before: u128,
    /// Actual successor financial State secure index.
    pub secure_index_after: u128,
    /// Actual predecessor software journal revision; distinct from secure index and `State.logical_sequence:u128`.
    pub logical_journal_sequence_before: u64,
    /// Actual successor software journal revision, not the financial State logical sequence.
    pub logical_journal_sequence_after: u64,
}
/// Data-only first-release ordinary `CashTerminalRecord` record.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryCashTerminalRecordV1")]
pub struct KagemushaOrdinaryCashTerminalRecordV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact immutable body selected before original purpose1 approval.
    pub body: KagemushaOrdinaryCashTerminalBodyV1,
    /// Same exact sender ordinary credential original selected by Native.
    pub sender_credential_digest: [u8; 32],
    /// Exact genuine purpose2 W plus its original selected complete Integrity lease.
    pub preparation_authorization_digest: [u8; 32],
    /// Exact genuine purpose1 W plus its original selected complete Integrity lease.
    pub terminal_authorization_digest: [u8; 32],
    /// SHA of the exact complete original purpose1 financial selection subject.
    pub terminal_subject_digest: [u8; 32],
    /// Actual Native admission interval; neither copied body time nor a caller clock.
    pub admission_clock_context: KagemushaOrdinaryCashClockContextV1,
    /// Exact original purpose1 W inclusive issuance bound.
    pub approval_issued_at_ms: u64,
    /// Exact original purpose1 W exclusive expiry, unchanged through recovery.
    pub approval_expires_at_ms: u64,
}
/// Exact ordinary receiver request original, with its actual platform signature evidence.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryPaymentRequestV1")]
pub struct KagemushaOrdinaryPaymentRequestV1 {
    /// Exact receiver signing subject, including its original interval context.
    pub body: KagemushaOrdinaryPaymentRequestBodyV1,
    /// Unmodified Android DER or Apple CBOR under its actual platform equation.
    pub evidence: KagemushaAppOperationApprovalEvidenceV1,
}
impl KagemushaOrdinaryCashClockContextV1 {
    /// Exact mathematical payload width, excluding domain and any request signing length.
    pub const PAYLOAD_BYTES: usize = 82;
    fn payload_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(&[]);
        t.raw("version", &self.version.to_le_bytes());
        t.raw("request_nonce", &self.request_nonce);
        t.raw(
            "signed_observations_original_digest",
            &self.signed_observations_original_digest,
        );
        t.raw("lower_at_ms", &self.lower_at_ms.to_le_bytes());
        t.raw("upper_at_ms", &self.upper_at_ms.to_le_bytes());
        t.finish()
    }
    /// Exact model-owned binding message and semantic ranges. This data projection validates
    /// no Native authority, clock observation signatures, recursive proofs or publication.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let payload = self.payload_transcript();
        let mut bytes = KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1.to_vec();
        let shift = bytes.len();
        bytes.extend_from_slice(&payload.bytes);
        KagemushaOrdinaryCashTranscriptV1 {
            bytes,
            fields: payload
                .fields
                .into_iter()
                .map(|mut field| {
                    field.range = shift + field.range.start..shift + field.range.end;
                    field
                })
                .collect(),
        }
    }
    /// SHA-256 of the exact fixed mathematical binding message, after shape validation.
    /// # Errors
    /// Rejects another version, operation, absent selector or malformed purpose-specific shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let transcript = self.binding_transcript();
        let payload_len = self.payload_transcript().bytes.len();
        if payload_len != Self::PAYLOAD_BYTES {
            return Err("ordinary cash transcript width differs".into());
        }
        Ok(transcript.digest())
    }
    /// Encode the sole bounded complete canonical Norito original after data-only shape checks.
    /// # Errors
    /// Rejects invalid shape, another codec layout or an oversized original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)
    }
    /// Decode one exact bounded first-release canonical Norito original, without authority.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical or invalid-shaped data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Complete-original identity, including all canonical schema, frame, field and CRC bytes.
    /// This does not authenticate an original or substitute for the mathematical binding digest.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Encoder-derived complete-original raw semantic positions. All frame and vector syntax
    /// remains pinned. This is layout metadata; a fixed-topology circuit must independently
    /// constrain every selected semantic field and realize the actual checksum and SHA.
    /// # Errors
    /// Rejects an invalid specimen or a changed field, schema, primitive or canonical layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field("version", &self.version.to_le_bytes(), |changed, index| {
            changed.version ^= 1_u16 << (8 * index)
        })?;
        layout.field("request_nonce", &self.request_nonce, |changed, index| {
            changed.request_nonce[index] ^= 1
        })?;
        layout.field(
            "signed_observations_original_digest",
            &self.signed_observations_original_digest,
            |changed, index| changed.signed_observations_original_digest[index] ^= 1,
        )?;
        layout.field(
            "lower_at_ms",
            &self.lower_at_ms.to_le_bytes(),
            |changed, index| changed.lower_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "upper_at_ms",
            &self.upper_at_ms.to_le_bytes(),
            |changed, index| changed.upper_at_ms ^= 1_u64 << (8 * index),
        )?;
        Ok(layout.finish())
    }
}
impl KagemushaOrdinaryPaymentRequestBodyV1 {
    /// Exact mathematical payload width, excluding domain and any request signing length.
    pub const PAYLOAD_BYTES: usize = 390;
    fn payload_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(&[]);
        t.raw("version", &self.version.to_le_bytes());
        t.raw("release_id", &self.release_id);
        t.raw("network_id", &self.network_id);
        t.raw("normalized_asset_id", &self.normalized_asset_id);
        t.raw("asset_incarnation", &self.asset_incarnation);
        t.raw("scale", &self.scale.to_le_bytes());
        t.raw("reserve_pool_id", &self.reserve_pool_id);
        t.raw("recipient_account_binding", &self.recipient_account_binding);
        t.raw("amount", &self.amount.to_le_bytes());
        t.raw("recipient_encryption_key", &self.recipient_encryption_key);
        t.raw(
            "recipient_credential_digest",
            &self.recipient_credential_digest,
        );
        t.raw("recipient_lane_id", &self.recipient_lane_id);
        t.raw("request_id", &self.request_id);
        t.raw(
            "clock_context_digest",
            &self.clock_context.binding_transcript().digest(),
        );
        t.raw("issued_at_ms", &self.issued_at_ms.to_le_bytes());
        t.raw("expires_at_ms", &self.expires_at_ms.to_le_bytes());
        t.finish()
    }
    /// Exact model-owned binding message and semantic ranges. This data projection validates
    /// no Native authority, clock observation signatures, recursive proofs or publication.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let payload = self.payload_transcript();
        let mut bytes = KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_DOMAIN_V1.to_vec();
        bytes.extend_from_slice(&(Self::PAYLOAD_BYTES as u64).to_le_bytes());
        let shift = bytes.len();
        bytes.extend_from_slice(&payload.bytes);
        KagemushaOrdinaryCashTranscriptV1 {
            bytes,
            fields: payload
                .fields
                .into_iter()
                .map(|mut field| {
                    field.range = shift + field.range.start..shift + field.range.end;
                    field
                })
                .collect(),
        }
    }
    /// SHA-256 of the exact fixed mathematical binding message, after shape validation.
    /// # Errors
    /// Rejects another version, operation, absent selector or malformed purpose-specific shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let transcript = self.binding_transcript();
        let payload_len = self.payload_transcript().bytes.len();
        if payload_len != Self::PAYLOAD_BYTES {
            return Err("ordinary cash transcript width differs".into());
        }
        Ok(transcript.digest())
    }
    /// Encode the sole bounded complete canonical Norito original after data-only shape checks.
    /// # Errors
    /// Rejects invalid shape, another codec layout or an oversized original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)
    }
    /// Decode one exact bounded first-release canonical Norito original, without authority.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical or invalid-shaped data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Complete-original identity, including all canonical schema, frame, field and CRC bytes.
    /// This does not authenticate an original or substitute for the mathematical binding digest.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Encoder-derived complete-original raw semantic positions. All frame and vector syntax
    /// remains pinned. This is layout metadata; a fixed-topology circuit must independently
    /// constrain every selected semantic field and realize the actual checksum and SHA.
    /// # Errors
    /// Rejects an invalid specimen or a changed field, schema, primitive or canonical layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field("version", &self.version.to_le_bytes(), |changed, index| {
            changed.version ^= 1_u16 << (8 * index)
        })?;
        layout.field("release_id", &self.release_id, |changed, index| {
            changed.release_id[index] ^= 1
        })?;
        layout.field("network_id", &self.network_id, |changed, index| {
            changed.network_id[index] ^= 1
        })?;
        layout.field(
            "normalized_asset_id",
            &self.normalized_asset_id,
            |changed, index| changed.normalized_asset_id[index] ^= 1,
        )?;
        layout.field(
            "asset_incarnation",
            &self.asset_incarnation,
            |changed, index| changed.asset_incarnation[index] ^= 1,
        )?;
        layout.field("scale", &self.scale.to_le_bytes(), |changed, index| {
            changed.scale ^= 1_u32 << (8 * index)
        })?;
        layout.field(
            "reserve_pool_id",
            &self.reserve_pool_id,
            |changed, index| changed.reserve_pool_id[index] ^= 1,
        )?;
        layout.field(
            "recipient_account_binding",
            &self.recipient_account_binding,
            |changed, index| changed.recipient_account_binding[index] ^= 1,
        )?;
        layout.field("amount", &self.amount.to_le_bytes(), |changed, index| {
            changed.amount ^= 1_u128 << (8 * index)
        })?;
        layout.field(
            "recipient_encryption_key",
            &self.recipient_encryption_key,
            |changed, index| changed.recipient_encryption_key[index] ^= 1,
        )?;
        layout.field(
            "recipient_credential_digest",
            &self.recipient_credential_digest,
            |changed, index| changed.recipient_credential_digest[index] ^= 1,
        )?;
        layout.field(
            "recipient_lane_id",
            &self.recipient_lane_id,
            |changed, index| changed.recipient_lane_id[index] ^= 1,
        )?;
        layout.field("request_id", &self.request_id, |changed, index| {
            changed.request_id[index] ^= 1
        })?;
        layout.field(
            "clock_context.version",
            &self.clock_context.version.to_le_bytes(),
            |changed, index| changed.clock_context.version ^= 1_u16 << (8 * index),
        )?;
        layout.field(
            "clock_context.request_nonce",
            &self.clock_context.request_nonce,
            |changed, index| changed.clock_context.request_nonce[index] ^= 1,
        )?;
        layout.field(
            "clock_context.signed_observations_original_digest",
            &self.clock_context.signed_observations_original_digest,
            |changed, index| changed.clock_context.signed_observations_original_digest[index] ^= 1,
        )?;
        layout.field(
            "clock_context.lower_at_ms",
            &self.clock_context.lower_at_ms.to_le_bytes(),
            |changed, index| changed.clock_context.lower_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "clock_context.upper_at_ms",
            &self.clock_context.upper_at_ms.to_le_bytes(),
            |changed, index| changed.clock_context.upper_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "issued_at_ms",
            &self.issued_at_ms.to_le_bytes(),
            |changed, index| changed.issued_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "expires_at_ms",
            &self.expires_at_ms.to_le_bytes(),
            |changed, index| changed.expires_at_ms ^= 1_u64 << (8 * index),
        )?;
        Ok(layout.finish())
    }
}
impl KagemushaOrdinaryPaymentOutputV1 {
    /// Exact mathematical payload width, excluding domain and any request signing length.
    pub const PAYLOAD_BYTES: usize = 282;
    fn payload_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(&[]);
        t.raw("version", &self.version.to_le_bytes());
        t.raw("request_digest", &self.request_digest);
        t.raw("amount", &self.amount.to_le_bytes());
        t.raw("sender_before_commitment", &self.sender_before_commitment);
        t.raw("sender_after_commitment", &self.sender_after_commitment);
        t.raw("transition_nullifier", &self.transition_nullifier);
        t.raw("credit_id", &self.credit_id);
        t.raw("ciphertext_commitment", &self.ciphertext_commitment);
        t.raw("encrypted_credit_digest", &self.encrypted_credit_digest);
        t.raw("clock_context_digest", &self.clock_context_digest);
        t.raw("prepared_at_ms", &self.prepared_at_ms.to_le_bytes());
        t.finish()
    }
    /// Exact model-owned binding message and semantic ranges. This data projection validates
    /// no Native authority, clock observation signatures, recursive proofs or publication.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let payload = self.payload_transcript();
        let mut bytes = KAGEMUSHA_ORDINARY_PAYMENT_OUTPUT_DOMAIN_V1.to_vec();
        let shift = bytes.len();
        bytes.extend_from_slice(&payload.bytes);
        KagemushaOrdinaryCashTranscriptV1 {
            bytes,
            fields: payload
                .fields
                .into_iter()
                .map(|mut field| {
                    field.range = shift + field.range.start..shift + field.range.end;
                    field
                })
                .collect(),
        }
    }
    /// SHA-256 of the exact fixed mathematical binding message, after shape validation.
    /// # Errors
    /// Rejects another version, operation, absent selector or malformed purpose-specific shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let transcript = self.binding_transcript();
        let payload_len = self.payload_transcript().bytes.len();
        if payload_len != Self::PAYLOAD_BYTES {
            return Err("ordinary cash transcript width differs".into());
        }
        Ok(transcript.digest())
    }
    /// Encode the sole bounded complete canonical Norito original after data-only shape checks.
    /// # Errors
    /// Rejects invalid shape, another codec layout or an oversized original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)
    }
    /// Decode one exact bounded first-release canonical Norito original, without authority.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical or invalid-shaped data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Complete-original identity, including all canonical schema, frame, field and CRC bytes.
    /// This does not authenticate an original or substitute for the mathematical binding digest.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Encoder-derived complete-original raw semantic positions. All frame and vector syntax
    /// remains pinned. This is layout metadata; a fixed-topology circuit must independently
    /// constrain every selected semantic field and realize the actual checksum and SHA.
    /// # Errors
    /// Rejects an invalid specimen or a changed field, schema, primitive or canonical layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field("version", &self.version.to_le_bytes(), |changed, index| {
            changed.version ^= 1_u16 << (8 * index)
        })?;
        layout.field("request_digest", &self.request_digest, |changed, index| {
            changed.request_digest[index] ^= 1
        })?;
        layout.field("amount", &self.amount.to_le_bytes(), |changed, index| {
            changed.amount ^= 1_u128 << (8 * index)
        })?;
        layout.field(
            "sender_before_commitment",
            &self.sender_before_commitment,
            |changed, index| changed.sender_before_commitment[index] ^= 1,
        )?;
        layout.field(
            "sender_after_commitment",
            &self.sender_after_commitment,
            |changed, index| changed.sender_after_commitment[index] ^= 1,
        )?;
        layout.field(
            "transition_nullifier",
            &self.transition_nullifier,
            |changed, index| changed.transition_nullifier[index] ^= 1,
        )?;
        layout.field("credit_id", &self.credit_id, |changed, index| {
            changed.credit_id[index] ^= 1
        })?;
        layout.field(
            "ciphertext_commitment",
            &self.ciphertext_commitment,
            |changed, index| changed.ciphertext_commitment[index] ^= 1,
        )?;
        layout.field(
            "encrypted_credit_digest",
            &self.encrypted_credit_digest,
            |changed, index| changed.encrypted_credit_digest[index] ^= 1,
        )?;
        layout.field(
            "clock_context_digest",
            &self.clock_context_digest,
            |changed, index| changed.clock_context_digest[index] ^= 1,
        )?;
        layout.field(
            "prepared_at_ms",
            &self.prepared_at_ms.to_le_bytes(),
            |changed, index| changed.prepared_at_ms ^= 1_u64 << (8 * index),
        )?;
        Ok(layout.finish())
    }
}
/// Derive the sole acyclic ordinary Send AAD from exact request and preparation data.
/// This is a data projection; it authenticates no receiver, clock or financial owner.
/// Candidate, ciphertext, platform approval and recursive proof bytes are deliberately absent.
/// # Errors
/// Rejects malformed request/clock, reserved selectors, unchanged State or a preparation interval
/// outside the exact signed receiver request. The amount comes only from that complete request.
pub fn kagemusha_ordinary_send_credit_aad_v1(
    request: &KagemushaOrdinaryPaymentRequestV1,
    sender_before_commitment: [u8; 32],
    sender_after_commitment: [u8; 32],
    transition_nullifier: [u8; 32],
    ciphertext_commitment: [u8; 32],
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<super::KagemushaEncryptedCreditAadV1, String> {
    let request_digest = request.canonical_original_digest()?;
    nonzero(&[
        sender_before_commitment,
        sender_after_commitment,
        transition_nullifier,
        ciphertext_commitment,
    ])?;
    if sender_before_commitment == sender_after_commitment {
        return Err("ordinary send credit State heads are unchanged".into());
    }
    clock.validate_within_original_window(request.body.issued_at_ms, request.body.expires_at_ms)?;
    let clock_digest = clock.binding_digest()?;
    let credit_id = kagemusha_ordinary_credit_id_v1(transition_nullifier, request_digest);
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-send-credit-context\0");
    hash.update(request.body.version.to_le_bytes());
    hash.update(request_digest);
    hash.update(request.body.amount.to_le_bytes());
    hash.update(sender_before_commitment);
    hash.update(sender_after_commitment);
    hash.update(transition_nullifier);
    hash.update(ciphertext_commitment);
    hash.update(clock_digest);
    hash.update(clock.upper_at_ms.to_le_bytes());
    let aad = super::KagemushaEncryptedCreditAadV1 {
        version: request.body.version,
        purpose: super::KagemushaEncryptedCreditPurposeV1::Peer,
        context_digest: hash.finalize().into(),
        issuance_or_transition_commitment: ciphertext_commitment,
        credit_id,
        amount: request.body.amount,
    };
    aad.validate_shape().map_err(|error| error.to_string())?;
    Ok(aad)
}

impl KagemushaOrdinaryPaymentOutputV1 {
    /// Reconstruct the same pre-encryption AAD for actual Native receiver decryption.
    /// The encrypted-byte digest remains an independently opened output field, never an AAD input.
    /// # Errors
    /// Rejects a substituted complete request, amount, clock, credit identity or output shape.
    pub fn encrypted_credit_aad_against(
        &self,
        request: &KagemushaOrdinaryPaymentRequestV1,
        clock: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<super::KagemushaEncryptedCreditAadV1, String> {
        self.validate_against_clock(clock)?;
        if self.request_digest != request.canonical_original_digest()?
            || self.amount != request.body.amount
        {
            return Err("ordinary send credit request original differs".into());
        }
        let aad = kagemusha_ordinary_send_credit_aad_v1(
            request,
            self.sender_before_commitment,
            self.sender_after_commitment,
            self.transition_nullifier,
            self.ciphertext_commitment,
            clock,
        )?;
        if aad.credit_id != self.credit_id {
            return Err("ordinary send credit identity differs".into());
        }
        Ok(aad)
    }
}

impl KagemushaOrdinaryPreparedTransitionV1 {
    /// Exact mathematical payload width, excluding domain and any request signing length.
    pub const PAYLOAD_BYTES: usize = 211;
    fn payload_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(&[]);
        t.raw("version", &self.version.to_le_bytes());
        t.raw("operation", &self.operation.to_le_bytes());
        t.raw("lifecycle_digest", &self.lifecycle_digest);
        t.raw("request_digest", &self.request_digest);
        t.raw("predecessor_state", &self.predecessor_state);
        t.raw("successor_state", &self.successor_state);
        t.raw("amount", &self.amount.to_le_bytes());
        t.raw("reservation_digest", &self.reservation_digest);
        t.raw(
            "native_preparation_operation_id",
            &self.native_preparation_operation_id,
        );
        t.finish()
    }
    /// Exact model-owned binding message and semantic ranges. This data projection validates
    /// no Native authority, clock observation signatures, recursive proofs or publication.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let payload = self.payload_transcript();
        let mut bytes = KAGEMUSHA_ORDINARY_PREPARED_TRANSITION_DOMAIN_V1.to_vec();
        let shift = bytes.len();
        bytes.extend_from_slice(&payload.bytes);
        KagemushaOrdinaryCashTranscriptV1 {
            bytes,
            fields: payload
                .fields
                .into_iter()
                .map(|mut field| {
                    field.range = shift + field.range.start..shift + field.range.end;
                    field
                })
                .collect(),
        }
    }
    /// SHA-256 of the exact fixed mathematical binding message, after shape validation.
    /// # Errors
    /// Rejects another version, operation, absent selector or malformed purpose-specific shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let transcript = self.binding_transcript();
        let payload_len = self.payload_transcript().bytes.len();
        if payload_len != Self::PAYLOAD_BYTES {
            return Err("ordinary cash transcript width differs".into());
        }
        Ok(transcript.digest())
    }
    /// Encode the sole bounded complete canonical Norito original after data-only shape checks.
    /// # Errors
    /// Rejects invalid shape, another codec layout or an oversized original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)
    }
    /// Decode one exact bounded first-release canonical Norito original, without authority.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical or invalid-shaped data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Complete-original identity, including all canonical schema, frame, field and CRC bytes.
    /// This does not authenticate an original or substitute for the mathematical binding digest.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Encoder-derived complete-original raw semantic positions. All frame and vector syntax
    /// remains pinned. This is layout metadata; a fixed-topology circuit must independently
    /// constrain every selected semantic field and realize the actual checksum and SHA.
    /// # Errors
    /// Rejects an invalid specimen or a changed field, schema, primitive or canonical layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field("version", &self.version.to_le_bytes(), |changed, index| {
            changed.version ^= 1_u16 << (8 * index)
        })?;
        layout.field(
            "operation",
            &self.operation.to_le_bytes(),
            |changed, index| changed.operation ^= 1_u8 << (8 * index),
        )?;
        layout.field(
            "lifecycle_digest",
            &self.lifecycle_digest,
            |changed, index| changed.lifecycle_digest[index] ^= 1,
        )?;
        layout.field("request_digest", &self.request_digest, |changed, index| {
            changed.request_digest[index] ^= 1
        })?;
        layout.field(
            "predecessor_state",
            &self.predecessor_state,
            |changed, index| changed.predecessor_state[index] ^= 1,
        )?;
        layout.field(
            "successor_state",
            &self.successor_state,
            |changed, index| changed.successor_state[index] ^= 1,
        )?;
        layout.field("amount", &self.amount.to_le_bytes(), |changed, index| {
            changed.amount ^= 1_u128 << (8 * index)
        })?;
        layout.field(
            "reservation_digest",
            &self.reservation_digest,
            |changed, index| changed.reservation_digest[index] ^= 1,
        )?;
        layout.field(
            "native_preparation_operation_id",
            &self.native_preparation_operation_id,
            |changed, index| changed.native_preparation_operation_id[index] ^= 1,
        )?;
        Ok(layout.finish())
    }
}
impl KagemushaOrdinaryPreparedOutgoingV1 {
    /// Exact mathematical payload width, excluding domain and any request signing length.
    pub const PAYLOAD_BYTES: usize = 435;
    fn payload_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(&[]);
        t.raw("version", &self.version.to_le_bytes());
        t.raw("operation", &self.operation.to_le_bytes());
        t.raw("predecessor_state", &self.predecessor_state);
        t.raw("successor_state", &self.successor_state);
        t.raw("transition_digest", &self.transition_digest);
        t.raw(
            "prepared_transition_binding_digest",
            &self.prepared_transition_binding_digest,
        );
        t.raw(
            "projection_semantic_digest",
            &self.projection_semantic_digest,
        );
        t.raw("lifecycle_binding_digest", &self.lifecycle_binding_digest);
        t.raw("request_digest", &self.request_digest);
        t.raw("artifact_manifest_digest", &self.artifact_manifest_digest);
        t.raw("preparation_guard_digest", &self.preparation_guard_digest);
        t.raw("reservation_digest", &self.reservation_digest);
        t.raw(
            "preparation_authorization_digest",
            &self.preparation_authorization_digest,
        );
        for index in 0..2 {
            t.raw(
                if index == 0 {
                    "transition_stream_length"
                } else {
                    "recovery_stream_length"
                },
                &self.stream_lengths[index].to_le_bytes(),
            );
            t.raw(
                if index == 0 {
                    "transition_stream_digest"
                } else {
                    "recovery_stream_digest"
                },
                &self.stream_digests[index],
            );
        }
        t.finish()
    }
    /// Exact model-owned binding message and semantic ranges. This data projection validates
    /// no Native authority, clock observation signatures, recursive proofs or publication.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let payload = self.payload_transcript();
        let mut bytes = KAGEMUSHA_ORDINARY_PREPARED_OUTGOING_DOMAIN_V1.to_vec();
        let shift = bytes.len();
        bytes.extend_from_slice(&payload.bytes);
        KagemushaOrdinaryCashTranscriptV1 {
            bytes,
            fields: payload
                .fields
                .into_iter()
                .map(|mut field| {
                    field.range = shift + field.range.start..shift + field.range.end;
                    field
                })
                .collect(),
        }
    }
    /// SHA-256 of the exact fixed mathematical binding message, after shape validation.
    /// # Errors
    /// Rejects another version, operation, absent selector or malformed purpose-specific shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let transcript = self.binding_transcript();
        let payload_len = self.payload_transcript().bytes.len();
        if payload_len != Self::PAYLOAD_BYTES {
            return Err("ordinary cash transcript width differs".into());
        }
        Ok(transcript.digest())
    }
    /// Encode the sole bounded complete canonical Norito original after data-only shape checks.
    /// # Errors
    /// Rejects invalid shape, another codec layout or an oversized original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)
    }
    /// Decode one exact bounded first-release canonical Norito original, without authority.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical or invalid-shaped data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Complete-original identity, including all canonical schema, frame, field and CRC bytes.
    /// This does not authenticate an original or substitute for the mathematical binding digest.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Encoder-derived complete-original raw semantic positions. All frame and vector syntax
    /// remains pinned. This is layout metadata; a fixed-topology circuit must independently
    /// constrain every selected semantic field and realize the actual checksum and SHA.
    /// # Errors
    /// Rejects an invalid specimen or a changed field, schema, primitive or canonical layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field("version", &self.version.to_le_bytes(), |changed, index| {
            changed.version ^= 1_u16 << (8 * index)
        })?;
        layout.field(
            "operation",
            &self.operation.to_le_bytes(),
            |changed, index| changed.operation ^= 1_u8 << (8 * index),
        )?;
        layout.field(
            "predecessor_state",
            &self.predecessor_state,
            |changed, index| changed.predecessor_state[index] ^= 1,
        )?;
        layout.field(
            "successor_state",
            &self.successor_state,
            |changed, index| changed.successor_state[index] ^= 1,
        )?;
        layout.field(
            "transition_digest",
            &self.transition_digest,
            |changed, index| changed.transition_digest[index] ^= 1,
        )?;
        layout.field(
            "prepared_transition_binding_digest",
            &self.prepared_transition_binding_digest,
            |changed, index| changed.prepared_transition_binding_digest[index] ^= 1,
        )?;
        layout.field(
            "projection_semantic_digest",
            &self.projection_semantic_digest,
            |changed, index| changed.projection_semantic_digest[index] ^= 1,
        )?;
        layout.field(
            "lifecycle_binding_digest",
            &self.lifecycle_binding_digest,
            |changed, index| changed.lifecycle_binding_digest[index] ^= 1,
        )?;
        layout.field("request_digest", &self.request_digest, |changed, index| {
            changed.request_digest[index] ^= 1
        })?;
        layout.field(
            "artifact_manifest_digest",
            &self.artifact_manifest_digest,
            |changed, index| changed.artifact_manifest_digest[index] ^= 1,
        )?;
        layout.field(
            "preparation_guard_digest",
            &self.preparation_guard_digest,
            |changed, index| changed.preparation_guard_digest[index] ^= 1,
        )?;
        layout.field(
            "reservation_digest",
            &self.reservation_digest,
            |changed, index| changed.reservation_digest[index] ^= 1,
        )?;
        layout.field(
            "preparation_authorization_digest",
            &self.preparation_authorization_digest,
            |changed, index| changed.preparation_authorization_digest[index] ^= 1,
        )?;
        layout.field(
            "stream_lengths.0",
            &self.stream_lengths[0].to_le_bytes(),
            |changed, index| changed.stream_lengths[0] ^= (1_u64) << (8 * index),
        )?;
        layout.field(
            "stream_lengths.1",
            &self.stream_lengths[1].to_le_bytes(),
            |changed, index| changed.stream_lengths[1] ^= (1_u64) << (8 * index),
        )?;
        layout.field(
            "stream_digests.0",
            &self.stream_digests[0],
            |changed, index| changed.stream_digests[0][index] ^= 1,
        )?;
        layout.field(
            "stream_digests.1",
            &self.stream_digests[1],
            |changed, index| changed.stream_digests[1][index] ^= 1,
        )?;
        Ok(layout.finish())
    }
}
impl KagemushaOrdinaryCashTerminalIntentV1 {
    /// Exact mathematical payload width, excluding domain and any request signing length.
    pub const PAYLOAD_BYTES: usize = 323;
    fn payload_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(&[]);
        t.raw("version", &self.version.to_le_bytes());
        t.raw("operation", &self.operation.to_le_bytes());
        t.raw("native_operation_id", &self.native_operation_id);
        t.raw("native_nonce", &self.native_nonce);
        t.raw("preparation_id", &self.preparation_id);
        t.raw("candidate_digest", &self.candidate_digest);
        t.raw("state_statement_digest", &self.state_statement_digest);
        t.raw(
            "predecessor_descriptor_prefix_digest",
            &self.predecessor_descriptor_prefix_digest,
        );
        t.raw("sender_credential_digest", &self.sender_credential_digest);
        t.raw("reservation_digest", &self.reservation_digest);
        t.raw(
            "secure_index_before",
            &self.secure_index_before.to_le_bytes(),
        );
        t.raw("secure_index_after", &self.secure_index_after.to_le_bytes());
        t.raw(
            "logical_journal_sequence_before",
            &self.logical_journal_sequence_before.to_le_bytes(),
        );
        t.raw(
            "logical_journal_sequence_after",
            &self.logical_journal_sequence_after.to_le_bytes(),
        );
        t.raw("issued_at_ms", &self.issued_at_ms.to_le_bytes());
        t.raw("expires_at_ms", &self.expires_at_ms.to_le_bytes());
        t.finish()
    }
    /// Exact model-owned binding message and semantic ranges. This data projection validates
    /// no Native authority, clock observation signatures, recursive proofs or publication.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let payload = self.payload_transcript();
        let mut bytes = KAGEMUSHA_ORDINARY_CASH_TERMINAL_INTENT_DOMAIN_V1.to_vec();
        let shift = bytes.len();
        bytes.extend_from_slice(&payload.bytes);
        KagemushaOrdinaryCashTranscriptV1 {
            bytes,
            fields: payload
                .fields
                .into_iter()
                .map(|mut field| {
                    field.range = shift + field.range.start..shift + field.range.end;
                    field
                })
                .collect(),
        }
    }
    /// SHA-256 of the exact fixed mathematical binding message, after shape validation.
    /// # Errors
    /// Rejects another version, operation, absent selector or malformed purpose-specific shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let transcript = self.binding_transcript();
        let payload_len = self.payload_transcript().bytes.len();
        if payload_len != Self::PAYLOAD_BYTES {
            return Err("ordinary cash transcript width differs".into());
        }
        Ok(transcript.digest())
    }
    /// Encode the sole bounded complete canonical Norito original after data-only shape checks.
    /// # Errors
    /// Rejects invalid shape, another codec layout or an oversized original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)
    }
    /// Decode one exact bounded first-release canonical Norito original, without authority.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical or invalid-shaped data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Complete-original identity, including all canonical schema, frame, field and CRC bytes.
    /// This does not authenticate an original or substitute for the mathematical binding digest.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Encoder-derived complete-original raw semantic positions. All frame and vector syntax
    /// remains pinned. This is layout metadata; a fixed-topology circuit must independently
    /// constrain every selected semantic field and realize the actual checksum and SHA.
    /// # Errors
    /// Rejects an invalid specimen or a changed field, schema, primitive or canonical layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field("version", &self.version.to_le_bytes(), |changed, index| {
            changed.version ^= 1_u16 << (8 * index)
        })?;
        layout.field(
            "operation",
            &self.operation.to_le_bytes(),
            |changed, index| changed.operation ^= 1_u8 << (8 * index),
        )?;
        layout.field(
            "native_operation_id",
            &self.native_operation_id,
            |changed, index| changed.native_operation_id[index] ^= 1,
        )?;
        layout.field("native_nonce", &self.native_nonce, |changed, index| {
            changed.native_nonce[index] ^= 1
        })?;
        layout.field("preparation_id", &self.preparation_id, |changed, index| {
            changed.preparation_id[index] ^= 1
        })?;
        layout.field(
            "candidate_digest",
            &self.candidate_digest,
            |changed, index| changed.candidate_digest[index] ^= 1,
        )?;
        layout.field(
            "state_statement_digest",
            &self.state_statement_digest,
            |changed, index| changed.state_statement_digest[index] ^= 1,
        )?;
        layout.field(
            "predecessor_descriptor_prefix_digest",
            &self.predecessor_descriptor_prefix_digest,
            |changed, index| changed.predecessor_descriptor_prefix_digest[index] ^= 1,
        )?;
        layout.field(
            "sender_credential_digest",
            &self.sender_credential_digest,
            |changed, index| changed.sender_credential_digest[index] ^= 1,
        )?;
        layout.field(
            "reservation_digest",
            &self.reservation_digest,
            |changed, index| changed.reservation_digest[index] ^= 1,
        )?;
        layout.field(
            "secure_index_before",
            &self.secure_index_before.to_le_bytes(),
            |changed, index| changed.secure_index_before ^= 1_u128 << (8 * index),
        )?;
        layout.field(
            "secure_index_after",
            &self.secure_index_after.to_le_bytes(),
            |changed, index| changed.secure_index_after ^= 1_u128 << (8 * index),
        )?;
        layout.field(
            "logical_journal_sequence_before",
            &self.logical_journal_sequence_before.to_le_bytes(),
            |changed, index| changed.logical_journal_sequence_before ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "logical_journal_sequence_after",
            &self.logical_journal_sequence_after.to_le_bytes(),
            |changed, index| changed.logical_journal_sequence_after ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "issued_at_ms",
            &self.issued_at_ms.to_le_bytes(),
            |changed, index| changed.issued_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "expires_at_ms",
            &self.expires_at_ms.to_le_bytes(),
            |changed, index| changed.expires_at_ms ^= 1_u64 << (8 * index),
        )?;
        Ok(layout.finish())
    }
}
impl KagemushaOrdinaryCashTerminalBodyV1 {
    /// Exact mathematical payload width, excluding domain and any request signing length.
    pub const PAYLOAD_BYTES: usize = 677;
    fn payload_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(&[]);
        t.raw("version", &self.version.to_le_bytes());
        t.raw("operation", &self.operation.to_le_bytes());
        t.raw("amount", &self.amount.to_le_bytes());
        t.raw("state_statement_digest", &self.state_statement_digest);
        t.raw("candidate_digest", &self.candidate_digest);
        t.raw("preparation_id", &self.preparation_id);
        t.raw(
            "prepared_projection_semantic_digest",
            &self.prepared_projection_semantic_digest,
        );
        t.raw("lifecycle_digest", &self.lifecycle_digest);
        t.raw("request_digest", &self.request_digest);
        t.raw(
            "recipient_credential_digest",
            &self.recipient_credential_digest,
        );
        t.raw("send_output_digest", &self.send_output_digest);
        t.raw("encrypted_credit_digest", &self.encrypted_credit_digest);
        t.raw("artifact_manifest_digest", &self.artifact_manifest_digest);
        t.raw("reservation_digest", &self.reservation_digest);
        t.raw("native_operation_id", &self.native_operation_id);
        t.raw("terminal_intent_digest", &self.terminal_intent_digest);
        t.raw(
            "predecessor_descriptor_prefix_digest",
            &self.predecessor_descriptor_prefix_digest,
        );
        for index in 0..2 {
            t.raw(
                if index == 0 {
                    "transition_stream_length"
                } else {
                    "recovery_stream_length"
                },
                &self.stream_lengths[index].to_le_bytes(),
            );
            t.raw(
                if index == 0 {
                    "transition_stream_digest"
                } else {
                    "recovery_stream_digest"
                },
                &self.stream_digests[index],
            );
        }
        t.embedded("clock_context", &self.clock_context.payload_transcript());
        t.raw(
            "secure_index_before",
            &self.secure_index_before.to_le_bytes(),
        );
        t.raw("secure_index_after", &self.secure_index_after.to_le_bytes());
        t.raw(
            "logical_journal_sequence_before",
            &self.logical_journal_sequence_before.to_le_bytes(),
        );
        t.raw(
            "logical_journal_sequence_after",
            &self.logical_journal_sequence_after.to_le_bytes(),
        );
        t.finish()
    }
    /// Exact model-owned binding message and semantic ranges. This data projection validates
    /// no Native authority, clock observation signatures, recursive proofs or publication.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let payload = self.payload_transcript();
        let mut bytes = KAGEMUSHA_ORDINARY_CASH_TERMINAL_BODY_DOMAIN_V1.to_vec();
        let shift = bytes.len();
        bytes.extend_from_slice(&payload.bytes);
        KagemushaOrdinaryCashTranscriptV1 {
            bytes,
            fields: payload
                .fields
                .into_iter()
                .map(|mut field| {
                    field.range = shift + field.range.start..shift + field.range.end;
                    field
                })
                .collect(),
        }
    }
    /// SHA-256 of the exact fixed mathematical binding message, after shape validation.
    /// # Errors
    /// Rejects another version, operation, absent selector or malformed purpose-specific shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let transcript = self.binding_transcript();
        let payload_len = self.payload_transcript().bytes.len();
        if payload_len != Self::PAYLOAD_BYTES {
            return Err("ordinary cash transcript width differs".into());
        }
        Ok(transcript.digest())
    }
    /// Encode the sole bounded complete canonical Norito original after data-only shape checks.
    /// # Errors
    /// Rejects invalid shape, another codec layout or an oversized original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)
    }
    /// Decode one exact bounded first-release canonical Norito original, without authority.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical or invalid-shaped data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Complete-original identity, including all canonical schema, frame, field and CRC bytes.
    /// This does not authenticate an original or substitute for the mathematical binding digest.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Encoder-derived complete-original raw semantic positions. All frame and vector syntax
    /// remains pinned. This is layout metadata; a fixed-topology circuit must independently
    /// constrain every selected semantic field and realize the actual checksum and SHA.
    /// # Errors
    /// Rejects an invalid specimen or a changed field, schema, primitive or canonical layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field("version", &self.version.to_le_bytes(), |changed, index| {
            changed.version ^= 1_u16 << (8 * index)
        })?;
        layout.field(
            "operation",
            &self.operation.to_le_bytes(),
            |changed, index| changed.operation ^= 1_u8 << (8 * index),
        )?;
        layout.field("amount", &self.amount.to_le_bytes(), |changed, index| {
            changed.amount ^= 1_u128 << (8 * index)
        })?;
        layout.field(
            "state_statement_digest",
            &self.state_statement_digest,
            |changed, index| changed.state_statement_digest[index] ^= 1,
        )?;
        layout.field(
            "candidate_digest",
            &self.candidate_digest,
            |changed, index| changed.candidate_digest[index] ^= 1,
        )?;
        layout.field("preparation_id", &self.preparation_id, |changed, index| {
            changed.preparation_id[index] ^= 1
        })?;
        layout.field(
            "prepared_projection_semantic_digest",
            &self.prepared_projection_semantic_digest,
            |changed, index| changed.prepared_projection_semantic_digest[index] ^= 1,
        )?;
        layout.field(
            "lifecycle_digest",
            &self.lifecycle_digest,
            |changed, index| changed.lifecycle_digest[index] ^= 1,
        )?;
        layout.field("request_digest", &self.request_digest, |changed, index| {
            changed.request_digest[index] ^= 1
        })?;
        layout.field(
            "recipient_credential_digest",
            &self.recipient_credential_digest,
            |changed, index| changed.recipient_credential_digest[index] ^= 1,
        )?;
        layout.field(
            "send_output_digest",
            &self.send_output_digest,
            |changed, index| changed.send_output_digest[index] ^= 1,
        )?;
        layout.field(
            "encrypted_credit_digest",
            &self.encrypted_credit_digest,
            |changed, index| changed.encrypted_credit_digest[index] ^= 1,
        )?;
        layout.field(
            "artifact_manifest_digest",
            &self.artifact_manifest_digest,
            |changed, index| changed.artifact_manifest_digest[index] ^= 1,
        )?;
        layout.field(
            "reservation_digest",
            &self.reservation_digest,
            |changed, index| changed.reservation_digest[index] ^= 1,
        )?;
        layout.field(
            "native_operation_id",
            &self.native_operation_id,
            |changed, index| changed.native_operation_id[index] ^= 1,
        )?;
        layout.field(
            "terminal_intent_digest",
            &self.terminal_intent_digest,
            |changed, index| changed.terminal_intent_digest[index] ^= 1,
        )?;
        layout.field(
            "predecessor_descriptor_prefix_digest",
            &self.predecessor_descriptor_prefix_digest,
            |changed, index| changed.predecessor_descriptor_prefix_digest[index] ^= 1,
        )?;
        layout.field(
            "stream_lengths.0",
            &self.stream_lengths[0].to_le_bytes(),
            |changed, index| changed.stream_lengths[0] ^= (1_u64) << (8 * index),
        )?;
        layout.field(
            "stream_lengths.1",
            &self.stream_lengths[1].to_le_bytes(),
            |changed, index| changed.stream_lengths[1] ^= (1_u64) << (8 * index),
        )?;
        layout.field(
            "stream_digests.0",
            &self.stream_digests[0],
            |changed, index| changed.stream_digests[0][index] ^= 1,
        )?;
        layout.field(
            "stream_digests.1",
            &self.stream_digests[1],
            |changed, index| changed.stream_digests[1][index] ^= 1,
        )?;
        layout.field(
            "clock_context.version",
            &self.clock_context.version.to_le_bytes(),
            |changed, index| changed.clock_context.version ^= 1_u16 << (8 * index),
        )?;
        layout.field(
            "clock_context.request_nonce",
            &self.clock_context.request_nonce,
            |changed, index| changed.clock_context.request_nonce[index] ^= 1,
        )?;
        layout.field(
            "clock_context.signed_observations_original_digest",
            &self.clock_context.signed_observations_original_digest,
            |changed, index| changed.clock_context.signed_observations_original_digest[index] ^= 1,
        )?;
        layout.field(
            "clock_context.lower_at_ms",
            &self.clock_context.lower_at_ms.to_le_bytes(),
            |changed, index| changed.clock_context.lower_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "clock_context.upper_at_ms",
            &self.clock_context.upper_at_ms.to_le_bytes(),
            |changed, index| changed.clock_context.upper_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "secure_index_before",
            &self.secure_index_before.to_le_bytes(),
            |changed, index| changed.secure_index_before ^= 1_u128 << (8 * index),
        )?;
        layout.field(
            "secure_index_after",
            &self.secure_index_after.to_le_bytes(),
            |changed, index| changed.secure_index_after ^= 1_u128 << (8 * index),
        )?;
        layout.field(
            "logical_journal_sequence_before",
            &self.logical_journal_sequence_before.to_le_bytes(),
            |changed, index| changed.logical_journal_sequence_before ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "logical_journal_sequence_after",
            &self.logical_journal_sequence_after.to_le_bytes(),
            |changed, index| changed.logical_journal_sequence_after ^= 1_u64 << (8 * index),
        )?;
        Ok(layout.finish())
    }
}
impl KagemushaOrdinaryCashTerminalRecordV1 {
    /// Exact mathematical payload width, excluding domain and any request signing length.
    pub const PAYLOAD_BYTES: usize = 905;
    fn payload_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let mut t = Transcript::new(&[]);
        t.raw("version", &self.version.to_le_bytes());
        t.embedded("body", &self.body.payload_transcript());
        t.raw("sender_credential_digest", &self.sender_credential_digest);
        t.raw(
            "preparation_authorization_digest",
            &self.preparation_authorization_digest,
        );
        t.raw(
            "terminal_authorization_digest",
            &self.terminal_authorization_digest,
        );
        t.raw("terminal_subject_digest", &self.terminal_subject_digest);
        t.embedded(
            "admission_clock_context",
            &self.admission_clock_context.payload_transcript(),
        );
        t.raw(
            "approval_issued_at_ms",
            &self.approval_issued_at_ms.to_le_bytes(),
        );
        t.raw(
            "approval_expires_at_ms",
            &self.approval_expires_at_ms.to_le_bytes(),
        );
        t.finish()
    }
    /// Exact model-owned binding message and semantic ranges. This data projection validates
    /// no Native authority, clock observation signatures, recursive proofs or publication.
    #[must_use]
    pub fn binding_transcript(&self) -> KagemushaOrdinaryCashTranscriptV1 {
        let payload = self.payload_transcript();
        let mut bytes = KAGEMUSHA_ORDINARY_CASH_TERMINAL_RECORD_DOMAIN_V1.to_vec();
        let shift = bytes.len();
        bytes.extend_from_slice(&payload.bytes);
        KagemushaOrdinaryCashTranscriptV1 {
            bytes,
            fields: payload
                .fields
                .into_iter()
                .map(|mut field| {
                    field.range = shift + field.range.start..shift + field.range.end;
                    field
                })
                .collect(),
        }
    }
    /// SHA-256 of the exact fixed mathematical binding message, after shape validation.
    /// # Errors
    /// Rejects another version, operation, absent selector or malformed purpose-specific shape.
    pub fn binding_digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        let transcript = self.binding_transcript();
        let payload_len = self.payload_transcript().bytes.len();
        if payload_len != Self::PAYLOAD_BYTES {
            return Err("ordinary cash transcript width differs".into());
        }
        Ok(transcript.digest())
    }
    /// Encode the sole bounded complete canonical Norito original after data-only shape checks.
    /// # Errors
    /// Rejects invalid shape, another codec layout or an oversized original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)
    }
    /// Decode one exact bounded first-release canonical Norito original, without authority.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical or invalid-shaped data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Complete-original identity, including all canonical schema, frame, field and CRC bytes.
    /// This does not authenticate an original or substitute for the mathematical binding digest.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Encoder-derived complete-original raw semantic positions. All frame and vector syntax
    /// remains pinned. This is layout metadata; a fixed-topology circuit must independently
    /// constrain every selected semantic field and realize the actual checksum and SHA.
    /// # Errors
    /// Rejects an invalid specimen or a changed field, schema, primitive or canonical layout.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field("version", &self.version.to_le_bytes(), |changed, index| {
            changed.version ^= 1_u16 << (8 * index)
        })?;
        layout.field(
            "body.version",
            &self.body.version.to_le_bytes(),
            |changed, index| changed.body.version ^= 1_u16 << (8 * index),
        )?;
        layout.field(
            "body.operation",
            &self.body.operation.to_le_bytes(),
            |changed, index| changed.body.operation ^= 1_u8 << (8 * index),
        )?;
        layout.field(
            "body.amount",
            &self.body.amount.to_le_bytes(),
            |changed, index| changed.body.amount ^= 1_u128 << (8 * index),
        )?;
        layout.field(
            "body.state_statement_digest",
            &self.body.state_statement_digest,
            |changed, index| changed.body.state_statement_digest[index] ^= 1,
        )?;
        layout.field(
            "body.candidate_digest",
            &self.body.candidate_digest,
            |changed, index| changed.body.candidate_digest[index] ^= 1,
        )?;
        layout.field(
            "body.preparation_id",
            &self.body.preparation_id,
            |changed, index| changed.body.preparation_id[index] ^= 1,
        )?;
        layout.field(
            "body.prepared_projection_semantic_digest",
            &self.body.prepared_projection_semantic_digest,
            |changed, index| changed.body.prepared_projection_semantic_digest[index] ^= 1,
        )?;
        layout.field(
            "body.lifecycle_digest",
            &self.body.lifecycle_digest,
            |changed, index| changed.body.lifecycle_digest[index] ^= 1,
        )?;
        layout.field(
            "body.request_digest",
            &self.body.request_digest,
            |changed, index| changed.body.request_digest[index] ^= 1,
        )?;
        layout.field(
            "body.recipient_credential_digest",
            &self.body.recipient_credential_digest,
            |changed, index| changed.body.recipient_credential_digest[index] ^= 1,
        )?;
        layout.field(
            "body.send_output_digest",
            &self.body.send_output_digest,
            |changed, index| changed.body.send_output_digest[index] ^= 1,
        )?;
        layout.field(
            "body.encrypted_credit_digest",
            &self.body.encrypted_credit_digest,
            |changed, index| changed.body.encrypted_credit_digest[index] ^= 1,
        )?;
        layout.field(
            "body.artifact_manifest_digest",
            &self.body.artifact_manifest_digest,
            |changed, index| changed.body.artifact_manifest_digest[index] ^= 1,
        )?;
        layout.field(
            "body.reservation_digest",
            &self.body.reservation_digest,
            |changed, index| changed.body.reservation_digest[index] ^= 1,
        )?;
        layout.field(
            "body.native_operation_id",
            &self.body.native_operation_id,
            |changed, index| changed.body.native_operation_id[index] ^= 1,
        )?;
        layout.field(
            "body.terminal_intent_digest",
            &self.body.terminal_intent_digest,
            |changed, index| changed.body.terminal_intent_digest[index] ^= 1,
        )?;
        layout.field(
            "body.predecessor_descriptor_prefix_digest",
            &self.body.predecessor_descriptor_prefix_digest,
            |changed, index| changed.body.predecessor_descriptor_prefix_digest[index] ^= 1,
        )?;
        layout.field(
            "body.stream_lengths.0",
            &self.body.stream_lengths[0].to_le_bytes(),
            |changed, index| changed.body.stream_lengths[0] ^= (1_u64) << (8 * index),
        )?;
        layout.field(
            "body.stream_lengths.1",
            &self.body.stream_lengths[1].to_le_bytes(),
            |changed, index| changed.body.stream_lengths[1] ^= (1_u64) << (8 * index),
        )?;
        layout.field(
            "body.stream_digests.0",
            &self.body.stream_digests[0],
            |changed, index| changed.body.stream_digests[0][index] ^= 1,
        )?;
        layout.field(
            "body.stream_digests.1",
            &self.body.stream_digests[1],
            |changed, index| changed.body.stream_digests[1][index] ^= 1,
        )?;
        layout.field(
            "body.clock_context.version",
            &self.body.clock_context.version.to_le_bytes(),
            |changed, index| changed.body.clock_context.version ^= 1_u16 << (8 * index),
        )?;
        layout.field(
            "body.clock_context.request_nonce",
            &self.body.clock_context.request_nonce,
            |changed, index| changed.body.clock_context.request_nonce[index] ^= 1,
        )?;
        layout.field(
            "body.clock_context.signed_observations_original_digest",
            &self.body.clock_context.signed_observations_original_digest,
            |changed, index| {
                changed
                    .body
                    .clock_context
                    .signed_observations_original_digest[index] ^= 1
            },
        )?;
        layout.field(
            "body.clock_context.lower_at_ms",
            &self.body.clock_context.lower_at_ms.to_le_bytes(),
            |changed, index| changed.body.clock_context.lower_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "body.clock_context.upper_at_ms",
            &self.body.clock_context.upper_at_ms.to_le_bytes(),
            |changed, index| changed.body.clock_context.upper_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "body.secure_index_before",
            &self.body.secure_index_before.to_le_bytes(),
            |changed, index| changed.body.secure_index_before ^= 1_u128 << (8 * index),
        )?;
        layout.field(
            "body.secure_index_after",
            &self.body.secure_index_after.to_le_bytes(),
            |changed, index| changed.body.secure_index_after ^= 1_u128 << (8 * index),
        )?;
        layout.field(
            "body.logical_journal_sequence_before",
            &self.body.logical_journal_sequence_before.to_le_bytes(),
            |changed, index| changed.body.logical_journal_sequence_before ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "body.logical_journal_sequence_after",
            &self.body.logical_journal_sequence_after.to_le_bytes(),
            |changed, index| changed.body.logical_journal_sequence_after ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "sender_credential_digest",
            &self.sender_credential_digest,
            |changed, index| changed.sender_credential_digest[index] ^= 1,
        )?;
        layout.field(
            "preparation_authorization_digest",
            &self.preparation_authorization_digest,
            |changed, index| changed.preparation_authorization_digest[index] ^= 1,
        )?;
        layout.field(
            "terminal_authorization_digest",
            &self.terminal_authorization_digest,
            |changed, index| changed.terminal_authorization_digest[index] ^= 1,
        )?;
        layout.field(
            "terminal_subject_digest",
            &self.terminal_subject_digest,
            |changed, index| changed.terminal_subject_digest[index] ^= 1,
        )?;
        layout.field(
            "admission_clock_context.version",
            &self.admission_clock_context.version.to_le_bytes(),
            |changed, index| changed.admission_clock_context.version ^= 1_u16 << (8 * index),
        )?;
        layout.field(
            "admission_clock_context.request_nonce",
            &self.admission_clock_context.request_nonce,
            |changed, index| changed.admission_clock_context.request_nonce[index] ^= 1,
        )?;
        layout.field(
            "admission_clock_context.signed_observations_original_digest",
            &self
                .admission_clock_context
                .signed_observations_original_digest,
            |changed, index| {
                changed
                    .admission_clock_context
                    .signed_observations_original_digest[index] ^= 1
            },
        )?;
        layout.field(
            "admission_clock_context.lower_at_ms",
            &self.admission_clock_context.lower_at_ms.to_le_bytes(),
            |changed, index| changed.admission_clock_context.lower_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "admission_clock_context.upper_at_ms",
            &self.admission_clock_context.upper_at_ms.to_le_bytes(),
            |changed, index| changed.admission_clock_context.upper_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "approval_issued_at_ms",
            &self.approval_issued_at_ms.to_le_bytes(),
            |changed, index| changed.approval_issued_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "approval_expires_at_ms",
            &self.approval_expires_at_ms.to_le_bytes(),
            |changed, index| changed.approval_expires_at_ms ^= 1_u64 << (8 * index),
        )?;
        Ok(layout.finish())
    }
}

impl KagemushaOrdinaryCashClockContextV1 {
    /// Check only interval ordering and original selectors. Signed observations, governed skew,
    /// continuity, durable high-water and freshness are exclusively checked by the Native owner.
    /// # Errors
    /// Rejects another version, absent nonce/original or empty/reversed Unix-ms bounds.
    pub fn validate_shape(&self) -> Result<(), String> {
        version(self.version)?;
        nonzero(&[self.request_nonce, self.signed_observations_original_digest])?;
        if self.lower_at_ms == 0 || self.lower_at_ms > self.upper_at_ms {
            return Err("ordinary cash clock interval differs".into());
        }
        Ok(())
    }
    /// Check the complete conservative interval lies in an original exclusive-expiry window.
    /// This is arithmetic only; a caller-supplied projection is never a trusted time source.
    /// # Errors
    /// Rejects invalid shape or either bound outside the unchanged original interval.
    pub fn validate_within_original_window(
        &self,
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> Result<(), String> {
        self.validate_shape()?;
        if issued_at_ms == 0
            || issued_at_ms >= expires_at_ms
            || self.lower_at_ms < issued_at_ms
            || self.upper_at_ms >= expires_at_ms
        {
            return Err("ordinary cash complete clock interval is outside original window".into());
        }
        Ok(())
    }
}
impl KagemushaOrdinaryPaymentRequestBodyV1 {
    /// Validate only signed request data, exact scalar bounds and its complete original interval.
    /// Native separately selects release, authoritative asset/pool, receiver C and current clock.
    /// # Errors
    /// Rejects absent selectors, a low-order encryption key, zero amount or an invalid lifetime.
    pub fn validate_shape(&self) -> Result<(), String> {
        version(self.version)?;
        nonzero(&[
            self.release_id,
            self.network_id,
            self.normalized_asset_id,
            self.asset_incarnation,
            self.reserve_pool_id,
            self.recipient_account_binding,
            self.recipient_credential_digest,
            self.recipient_lane_id,
            self.request_id,
        ])?;
        if self.amount == 0
            || self.scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
            || self.issued_at_ms == 0
            || self.issued_at_ms >= self.expires_at_ms
            || self.expires_at_ms - self.issued_at_ms > KAGEMUSHA_REQUEST_MAX_TTL_MS_V1
        {
            return Err("ordinary receiver request amount/scale/lifetime differs".into());
        }
        X25519Sha256::decode_public_key(&self.recipient_encryption_key)
            .map_err(|_| "ordinary receiver encryption key invalid")?;
        self.clock_context
            .validate_within_original_window(self.issued_at_ms, self.expires_at_ms)
    }
    /// Return the exact platform signing message, with LE64(390) and model-owned ranges.
    /// # Errors
    /// Rejects invalid request shape or another signing width.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        let value = self.binding_transcript();
        if self.payload_transcript().bytes.len() != Self::PAYLOAD_BYTES {
            return Err("ordinary request signing width differs".into());
        }
        Ok(value.bytes)
    }
}
impl KagemushaOrdinaryPaymentOutputV1 {
    /// Validate the raw pre-candidate send output, without a recursive proof or terminal claim.
    /// # Errors
    /// Rejects absent selectors, zero amount/time, unchanged State or a different ordinary credit ID.
    pub fn validate_shape(&self) -> Result<(), String> {
        version(self.version)?;
        nonzero(&[
            self.request_digest,
            self.sender_before_commitment,
            self.sender_after_commitment,
            self.transition_nullifier,
            self.credit_id,
            self.ciphertext_commitment,
            self.encrypted_credit_digest,
            self.clock_context_digest,
        ])?;
        if self.amount == 0
            || self.prepared_at_ms == 0
            || self.sender_before_commitment == self.sender_after_commitment
            || self.credit_id
                != kagemusha_ordinary_credit_id_v1(self.transition_nullifier, self.request_digest)
        {
            return Err("ordinary send output amount/state/credit/preparation time differs".into());
        }
        Ok(())
    }
    /// Join this preparation projection to one independently selected interval context.
    /// This join is data only and does not authenticate the clock context.
    /// # Errors
    /// Rejects invalid shape, substituted context or a time other than its conservative upper bound.
    pub fn validate_against_clock(
        &self,
        clock: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        if self.clock_context_digest != clock.binding_digest()?
            || self.prepared_at_ms != clock.upper_at_ms
        {
            return Err("ordinary send output clock projection differs".into());
        }
        Ok(())
    }
}
impl KagemushaOrdinaryPreparedTransitionV1 {
    /// Validate the acyclic pre-approval transition subject without creating financial custody.
    /// # Errors
    /// Rejects an absent scope, amount, Native operation or wrong send/redemption slot.
    pub fn validate_shape(&self) -> Result<(), String> {
        version(self.version)?;
        outgoing(self.operation)?;
        nonzero(&[
            self.lifecycle_digest,
            self.predecessor_state,
            self.successor_state,
            self.reservation_digest,
            self.native_preparation_operation_id,
        ])?;
        operation_slot(self.operation, self.request_digest, true)?;
        if self.amount == 0 || self.predecessor_state == self.successor_state {
            return Err("ordinary prepared transition amount/state differs".into());
        }
        Ok(())
    }
}
impl KagemushaOrdinaryPreparedOutgoingV1 {
    /// Validate data carried after purpose2 approval. Only genuine Guard/proof verification and
    /// the exclusive Native financial owner can give these original digests authenticated meaning.
    /// # Errors
    /// Rejects another operation, absent selectors, wrong purpose slots or sealed stream lengths.
    pub fn validate_shape(&self) -> Result<(), String> {
        version(self.version)?;
        outgoing(self.operation)?;
        nonzero(&[
            self.predecessor_state,
            self.successor_state,
            self.transition_digest,
            self.prepared_transition_binding_digest,
            self.projection_semantic_digest,
            self.lifecycle_binding_digest,
            self.preparation_guard_digest,
            self.reservation_digest,
            self.preparation_authorization_digest,
        ])?;
        if self.predecessor_state == self.successor_state {
            return Err("ordinary prepared outgoing State did not advance".into());
        }
        operation_slot(self.operation, self.request_digest, true)?;
        operation_slot(self.operation, self.artifact_manifest_digest, false)?;
        stream_bounds(self.stream_lengths, self.stream_digests)
    }
    /// Bind exact earlier pre-approval scope to the later prepared data.
    /// No approval signature, recursive proof or Native money capability is authenticated here.
    /// # Errors
    /// Rejects substituted operation, heads, lifecycle, request, reservation or pre-approval SHA.
    // The prepared lifecycle binding must equal the original transition lifecycle digest.
    #[allow(clippy::suspicious_operation_groupings)]
    pub fn validate_against_transition(
        &self,
        transition: &KagemushaOrdinaryPreparedTransitionV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        transition.validate_shape()?;
        let lifecycle_differs = self.lifecycle_binding_digest != transition.lifecycle_digest;
        if self.operation != transition.operation
            || self.predecessor_state != transition.predecessor_state
            || self.successor_state != transition.successor_state
            || lifecycle_differs
            || self.request_digest != transition.request_digest
            || self.reservation_digest != transition.reservation_digest
            || self.prepared_transition_binding_digest != transition.binding_digest()?
        {
            return Err("ordinary prepared record differs from pre-approval selection".into());
        }
        Ok(())
    }
}
impl KagemushaOrdinaryCashTerminalIntentV1 {
    /// Validate the original pre-W1 intent, preserving separate secure index and journal sequence.
    /// This record contains data and cannot reserve an OS call, journal slot or Native nonce itself.
    /// # Errors
    /// Rejects absent selectors, wrong operation, non-successor indexes or an oversized W1 interval.
    pub fn validate_shape(&self) -> Result<(), String> {
        version(self.version)?;
        outgoing(self.operation)?;
        nonzero(&[
            self.native_operation_id,
            self.native_nonce,
            self.preparation_id,
            self.candidate_digest,
            self.state_statement_digest,
            self.predecessor_descriptor_prefix_digest,
            self.sender_credential_digest,
            self.reservation_digest,
        ])?;
        index_step(
            self.secure_index_before,
            self.secure_index_after,
            self.logical_journal_sequence_before,
            self.logical_journal_sequence_after,
        )?;
        if self.issued_at_ms == 0
            || self.issued_at_ms >= self.expires_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > super::KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
        {
            return Err("ordinary terminal original approval interval differs".into());
        }
        Ok(())
    }
}
impl KagemushaOrdinaryCashTerminalBodyV1 {
    /// Validate pre-W1 body data. The exact candidate, State, output and both Guards still need
    /// actual verification; neither a body SHA nor a field named Native grants publication.
    /// # Errors
    /// Rejects missing selectors, incorrect purpose slots, non-successor indexes or sealed bounds.
    pub fn validate_shape(&self) -> Result<(), String> {
        version(self.version)?;
        outgoing(self.operation)?;
        nonzero(&[
            self.state_statement_digest,
            self.candidate_digest,
            self.preparation_id,
            self.prepared_projection_semantic_digest,
            self.lifecycle_digest,
            self.reservation_digest,
            self.native_operation_id,
            self.terminal_intent_digest,
            self.predecessor_descriptor_prefix_digest,
        ])?;
        if self.amount == 0 {
            return Err("ordinary terminal amount absent".into());
        }
        for value in [
            self.request_digest,
            self.recipient_credential_digest,
            self.send_output_digest,
            self.encrypted_credit_digest,
        ] {
            operation_slot(self.operation, value, true)?;
        }
        operation_slot(self.operation, self.artifact_manifest_digest, false)?;
        if self.operation == 2
            && self.prepared_projection_semantic_digest
                != kagemusha_ordinary_payment_body_digest_v1(
                    self.send_output_digest,
                    self.encrypted_credit_digest,
                )?
        {
            return Err("ordinary terminal pre-candidate send semantic differs".into());
        }
        stream_bounds(self.stream_lengths, self.stream_digests)?;
        self.clock_context.validate_shape()?;
        index_step(
            self.secure_index_before,
            self.secure_index_after,
            self.logical_journal_sequence_before,
            self.logical_journal_sequence_after,
        )
    }
    /// Join exact pre-W1 intent including nonce/sender C through its opened digest.
    /// The Native owner must additionally compare both records to the actual financial selection.
    /// # Errors
    /// Rejects any differing operation, candidate, State, preparation, original interval or indexes.
    pub fn validate_against_intent(
        &self,
        intent: &KagemushaOrdinaryCashTerminalIntentV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        intent.validate_shape()?;
        if self.operation != intent.operation
            || self.state_statement_digest != intent.state_statement_digest
            || self.candidate_digest != intent.candidate_digest
            || self.preparation_id != intent.preparation_id
            || self.native_operation_id != intent.native_operation_id
            || self.reservation_digest != intent.reservation_digest
            || self.predecessor_descriptor_prefix_digest
                != intent.predecessor_descriptor_prefix_digest
            || self.secure_index_before != intent.secure_index_before
            || self.secure_index_after != intent.secure_index_after
            || self.logical_journal_sequence_before != intent.logical_journal_sequence_before
            || self.logical_journal_sequence_after != intent.logical_journal_sequence_after
            || self.terminal_intent_digest != intent.binding_digest()?
        {
            return Err("ordinary terminal body differs from original intent".into());
        }
        self.clock_context
            .validate_within_original_window(intent.issued_at_ms, intent.expires_at_ms)
    }
    /// Join the exact purpose2 prepared data after it has been independently proof-verified.
    /// Data comparison alone never verifies its retained approvals or proof.
    /// # Errors
    /// Rejects a substituted preparation ID, operation, projection, lifecycle, request or streams.
    pub fn validate_against_prepared(
        &self,
        prepared: &KagemushaOrdinaryPreparedOutgoingV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        prepared.validate_shape()?;
        if self.preparation_id != prepared.binding_digest()?
            || self.operation != prepared.operation
            || self.state_statement_digest != prepared.transition_digest
            || self.prepared_projection_semantic_digest != prepared.projection_semantic_digest
            || self.lifecycle_digest != prepared.lifecycle_binding_digest
            || self.request_digest != prepared.request_digest
            || self.artifact_manifest_digest != prepared.artifact_manifest_digest
            || self.reservation_digest != prepared.reservation_digest
            || self.stream_lengths != prepared.stream_lengths
            || self.stream_digests != prepared.stream_digests
        {
            return Err("ordinary terminal body differs from purpose2 preparation".into());
        }
        Ok(())
    }
}
impl KagemushaOrdinaryCashTerminalRecordV1 {
    /// Validate post-approval logical record data and the complete admission interval in W1.
    /// This is not a hardware certificate or Native money grant; actual W1/nonce/PI/State/proof
    /// and durable original WAL custody are mandatory independent inputs to the Native owner.
    /// # Errors
    /// Rejects absent originals, another body or an admission interval outside original W1 expiry.
    pub fn validate_shape(&self) -> Result<(), String> {
        version(self.version)?;
        self.body.validate_shape()?;
        nonzero(&[
            self.sender_credential_digest,
            self.preparation_authorization_digest,
            self.terminal_authorization_digest,
            self.terminal_subject_digest,
        ])?;
        if self.approval_issued_at_ms == 0
            || self.approval_issued_at_ms >= self.approval_expires_at_ms
            || self.approval_expires_at_ms - self.approval_issued_at_ms
                > super::KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
        {
            return Err("ordinary terminal record W1 lifetime differs".into());
        }
        self.admission_clock_context
            .validate_within_original_window(
                self.approval_issued_at_ms,
                self.approval_expires_at_ms,
            )?;
        if self.admission_clock_context.lower_at_ms < self.body.clock_context.lower_at_ms
            || self.admission_clock_context.upper_at_ms < self.body.clock_context.upper_at_ms
        {
            return Err("ordinary terminal admission interval precedes body selection".into());
        }
        Ok(())
    }
    /// Join this exact logical record to the retained immutable original intent and purpose2 data.
    /// This validates data only. Native separately verifies authentic W1 purpose1 and original nonce.
    /// # Errors
    /// Rejects different original credentials, prepared authorization or approval interval.
    // The approved interval must exactly match the original intent issue and expiry times.
    #[allow(clippy::suspicious_operation_groupings)]
    pub fn validate_against_originals(
        &self,
        intent: &KagemushaOrdinaryCashTerminalIntentV1,
        prepared: &KagemushaOrdinaryPreparedOutgoingV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        self.body.validate_against_intent(intent)?;
        self.body.validate_against_prepared(prepared)?;
        let approval_window_differs = self.approval_issued_at_ms != intent.issued_at_ms
            || self.approval_expires_at_ms != intent.expires_at_ms;
        if self.sender_credential_digest != intent.sender_credential_digest
            || self.preparation_authorization_digest != prepared.preparation_authorization_digest
            || approval_window_differs
        {
            return Err("ordinary terminal record original selection differs".into());
        }
        Ok(())
    }
}
impl KagemushaOrdinaryPaymentRequestV1 {
    /// Check bounded exact original evidence grammar without authenticating the receiver or time.
    /// # Errors
    /// Rejects another request shape, malformed DER/CBOR or original evidence bound.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.body.validate_shape()?;
        match &self.evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                if !(8..=72).contains(&signature_der.len()) {
                    return Err("ordinary request DER bound differs".into());
                }
                p256::ecdsa::Signature::from_der(signature_der)
                    .map_err(|_| "ordinary request original DER malformed")?;
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                if raw_assertion.is_empty()
                    || raw_assertion.len() > KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1
                {
                    return Err("ordinary request App Attest original bound differs".into());
                }
                super::kagemusha_app_attest_original_counter_v1(raw_assertion)?;
            }
        }
        Ok(())
    }
    /// Encode the sole bounded complete canonical signed request original; no receiver admission.
    /// # Errors
    /// Rejects invalid data/evidence grammar or a canonical encoding error.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded_encode(self, KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_MAX_BYTES_V1)
    }
    /// Decode only the exact bounded first-release signed request original, without admission.
    /// # Errors
    /// Rejects trailing, oversized, noncanonical, wrong-shape or malformed evidence data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = exact_decode(bytes, KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        value.validate_shape()?;
        Ok(value)
    }
    /// Digest the complete exact canonical signed request, including raw platform evidence and CRC.
    /// # Errors
    /// Rejects invalid shape or canonical encoding.
    pub fn canonical_original_digest(&self) -> Result<[u8; 32], String> {
        Ok(original_digest(&self.canonical_bytes()?))
    }
    /// Verify the actual receiver platform equation over the sole ordinary request signing message.
    /// It verifies only the signature and same original C scope. Native must separately check C
    /// issuer/current release/FI/PI/current clock, request identity replay and exclusive custody.
    /// # Errors
    /// Rejects a different C/account/lane/release/network, stale independent Apple floor or signature.
    pub fn authenticate_receiver_signature(
        &self,
        receiver: &KagemushaVerifiedOrdinaryAppCredentialV1,
        independent_apple_counter_floor: Option<u32>,
    ) -> Result<(Option<u32>, Option<KagemushaAppAttestReleaseMeasurementV1>), String> {
        self.validate_shape()?;
        let c = receiver.subject();
        if self.body.recipient_credential_digest != receiver.digest()
            || self.body.release_id != c.release_id
            || self.body.network_id != c.network_id
            || self.body.recipient_lane_id != c.lane_id
            || self.body.recipient_account_binding != c.account_binding
        {
            return Err("ordinary receiver request differs from original C scope".into());
        }
        match (c.platform_class, independent_apple_counter_floor) {
            (KagemushaHardwarePlatformClassV1::AndroidKeyMint, None) => (),
            (KagemushaHardwarePlatformClassV1::AppleAppAttest, Some(floor))
                if floor >= c.app_attest_counter_floor => {}
            _ => return Err("ordinary receiver request original counter floor differs".into()),
        }
        self.evidence.authenticate_signature(
            c.platform_class,
            &c.app_public_key,
            c.app_signing_identity_digest,
            c.app_release_digest,
            independent_apple_counter_floor,
            &self.body.canonical_signing_bytes()?,
        )
    }
}

impl KagemushaOrdinaryPaymentRequestV1 {
    /// Derive raw semantic/evidence positions from this exact sole canonical request encoder.
    /// The actual platform discriminant, original width and sequence framing remain pinned;
    /// this specimen metadata cannot itself select a release circuit or authorize a receiver.
    /// # Errors
    /// Rejects invalid specimen shape or a changed model/schema/field encoding.
    pub fn original_preimage_layout(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        self.validate_shape()?;
        self.original_preimage_layout_for_specimen()
    }
    // Codec-only templates cannot authorize or authenticate original platform evidence.
    fn original_preimage_layout_for_specimen(
        &self,
    ) -> Result<KagemushaOrdinaryCashOriginalLayoutV1, String> {
        let mut layout = OriginalBuilder::new(self, KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1)?;
        layout.field(
            "version",
            &self.body.version.to_le_bytes(),
            |changed, index| changed.body.version ^= 1_u16 << (8 * index),
        )?;
        layout.field("release_id", &self.body.release_id, |changed, index| {
            changed.body.release_id[index] ^= 1
        })?;
        layout.field("network_id", &self.body.network_id, |changed, index| {
            changed.body.network_id[index] ^= 1
        })?;
        layout.field(
            "normalized_asset_id",
            &self.body.normalized_asset_id,
            |changed, index| changed.body.normalized_asset_id[index] ^= 1,
        )?;
        layout.field(
            "asset_incarnation",
            &self.body.asset_incarnation,
            |changed, index| changed.body.asset_incarnation[index] ^= 1,
        )?;
        layout.field("scale", &self.body.scale.to_le_bytes(), |changed, index| {
            changed.body.scale ^= 1_u32 << (8 * index)
        })?;
        layout.field(
            "reserve_pool_id",
            &self.body.reserve_pool_id,
            |changed, index| changed.body.reserve_pool_id[index] ^= 1,
        )?;
        layout.field(
            "recipient_account_binding",
            &self.body.recipient_account_binding,
            |changed, index| changed.body.recipient_account_binding[index] ^= 1,
        )?;
        layout.field(
            "amount",
            &self.body.amount.to_le_bytes(),
            |changed, index| changed.body.amount ^= 1_u128 << (8 * index),
        )?;
        layout.field(
            "recipient_encryption_key",
            &self.body.recipient_encryption_key,
            |changed, index| changed.body.recipient_encryption_key[index] ^= 1,
        )?;
        layout.field(
            "recipient_credential_digest",
            &self.body.recipient_credential_digest,
            |changed, index| changed.body.recipient_credential_digest[index] ^= 1,
        )?;
        layout.field(
            "recipient_lane_id",
            &self.body.recipient_lane_id,
            |changed, index| changed.body.recipient_lane_id[index] ^= 1,
        )?;
        layout.field("request_id", &self.body.request_id, |changed, index| {
            changed.body.request_id[index] ^= 1
        })?;
        layout.field(
            "clock_context.version",
            &self.body.clock_context.version.to_le_bytes(),
            |changed, index| changed.body.clock_context.version ^= 1_u16 << (8 * index),
        )?;
        layout.field(
            "clock_context.request_nonce",
            &self.body.clock_context.request_nonce,
            |changed, index| changed.body.clock_context.request_nonce[index] ^= 1,
        )?;
        layout.field(
            "clock_context.signed_observations_original_digest",
            &self.body.clock_context.signed_observations_original_digest,
            |changed, index| {
                changed
                    .body
                    .clock_context
                    .signed_observations_original_digest[index] ^= 1
            },
        )?;
        layout.field(
            "clock_context.lower_at_ms",
            &self.body.clock_context.lower_at_ms.to_le_bytes(),
            |changed, index| changed.body.clock_context.lower_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "clock_context.upper_at_ms",
            &self.body.clock_context.upper_at_ms.to_le_bytes(),
            |changed, index| changed.body.clock_context.upper_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "issued_at_ms",
            &self.body.issued_at_ms.to_le_bytes(),
            |changed, index| changed.body.issued_at_ms ^= 1_u64 << (8 * index),
        )?;
        layout.field(
            "expires_at_ms",
            &self.body.expires_at_ms.to_le_bytes(),
            |changed, index| changed.body.expires_at_ms ^= 1_u64 << (8 * index),
        )?;
        match &self.evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                layout.field("evidence.signature_der", signature_der, |changed, index| {
                    if let KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                        signature_der,
                    } = &mut changed.evidence
                    {
                        signature_der[index] ^= 1;
                    }
                })?;
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                layout.field("evidence.raw_assertion", raw_assertion, |changed, index| {
                    if let KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                        raw_assertion,
                    } = &mut changed.evidence
                    {
                        raw_assertion[index] ^= 1;
                    }
                })?;
            }
        }
        Ok(layout.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{SigningKey, signature::Signer as _};

    fn d(value: u8) -> [u8; 32] {
        [value; 32]
    }
    fn clock() -> KagemushaOrdinaryCashClockContextV1 {
        KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: d(1),
            signed_observations_original_digest: d(2),
            lower_at_ms: 1001,
            upper_at_ms: 1002,
        }
    }
    fn request_body() -> KagemushaOrdinaryPaymentRequestBodyV1 {
        let mut encryption = [0; 32];
        encryption[0] = 9;
        KagemushaOrdinaryPaymentRequestBodyV1 {
            version: 1,
            release_id: d(3),
            network_id: d(4),
            normalized_asset_id: d(5),
            asset_incarnation: d(6),
            scale: 2,
            reserve_pool_id: d(7),
            recipient_account_binding: d(8),
            amount: 17,
            recipient_encryption_key: encryption,
            recipient_credential_digest: d(9),
            recipient_lane_id: d(10),
            request_id: d(11),
            clock_context: clock(),
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        }
    }
    fn request() -> (KagemushaOrdinaryPaymentRequestV1, SigningKey) {
        let key = SigningKey::from_slice(&[1; 32]).unwrap();
        let body = request_body();
        let signature: p256::ecdsa::Signature = key.sign(&body.canonical_signing_bytes().unwrap());
        (
            KagemushaOrdinaryPaymentRequestV1 {
                body,
                evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                    signature_der: signature.to_der().as_bytes().to_vec(),
                },
            },
            key,
        )
    }
    #[test]
    fn ordinary_send_aad_is_acyclic_and_joins_exact_request_and_clock() {
        // Genuine cryptography over synthetic model vectors; no issuer/Native authority fixture.
        let request = request().0;
        let output = output();
        let aad = output
            .encrypted_credit_aad_against(&request, &clock())
            .unwrap();
        let mut altered = output;
        altered.encrypted_credit_digest[0] ^= 1;
        assert_eq!(
            aad,
            altered
                .encrypted_credit_aad_against(&request, &clock())
                .unwrap()
        );
        assert_ne!(
            output.binding_digest().unwrap(),
            altered.binding_digest().unwrap()
        );
        for selector in 0..5 {
            let mut changed = output;
            match selector {
                0 => changed.sender_before_commitment[0] ^= 1,
                1 => changed.sender_after_commitment[0] ^= 1,
                2 => {
                    changed.transition_nullifier[0] ^= 1;
                    changed.credit_id = kagemusha_ordinary_credit_id_v1(
                        changed.transition_nullifier,
                        changed.request_digest,
                    );
                }
                3 => changed.ciphertext_commitment[0] ^= 1,
                _ => {
                    let mut later = clock();
                    later.upper_at_ms += 1;
                    changed.clock_context_digest = later.binding_digest().unwrap();
                    changed.prepared_at_ms = later.upper_at_ms;
                    assert_ne!(
                        aad,
                        changed
                            .encrypted_credit_aad_against(&request, &later)
                            .unwrap()
                    );
                    continue;
                }
            }
            assert_ne!(
                aad,
                changed
                    .encrypted_credit_aad_against(&request, &clock())
                    .unwrap()
            );
        }
        let mut substituted = request;
        substituted.body.request_id[0] ^= 1;
        assert!(
            output
                .encrypted_credit_aad_against(&substituted, &clock())
                .is_err()
        );
        let mut expired = clock();
        expired.upper_at_ms = substituted.body.expires_at_ms;
        assert!(
            kagemusha_ordinary_send_credit_aad_v1(
                &substituted,
                d(12),
                d(13),
                d(14),
                d(15),
                &expired
            )
            .is_err()
        );
        assert!(
            kagemusha_ordinary_send_credit_aad_v1(
                &substituted,
                d(12),
                d(12),
                d(14),
                d(15),
                &clock()
            )
            .is_err()
        );
    }

    fn output() -> KagemushaOrdinaryPaymentOutputV1 {
        let request_digest = request().0.canonical_original_digest().unwrap();
        let nullifier = d(14);
        KagemushaOrdinaryPaymentOutputV1 {
            version: 1,
            request_digest,
            amount: 17,
            sender_before_commitment: d(12),
            sender_after_commitment: d(13),
            transition_nullifier: nullifier,
            credit_id: kagemusha_ordinary_credit_id_v1(nullifier, request_digest),
            ciphertext_commitment: d(15),
            encrypted_credit_digest: d(16),
            clock_context_digest: clock().binding_digest().unwrap(),
            prepared_at_ms: 1002,
        }
    }
    fn transition(operation: u8) -> KagemushaOrdinaryPreparedTransitionV1 {
        KagemushaOrdinaryPreparedTransitionV1 {
            version: 1,
            operation,
            lifecycle_digest: d(17),
            request_digest: if operation == 2 {
                output().request_digest
            } else {
                [0; 32]
            },
            predecessor_state: d(12),
            successor_state: d(13),
            amount: 17,
            reservation_digest: d(18),
            native_preparation_operation_id: d(19),
        }
    }
    fn prepared(operation: u8) -> KagemushaOrdinaryPreparedOutgoingV1 {
        let t = transition(operation);
        let o = output();
        KagemushaOrdinaryPreparedOutgoingV1 {
            version: 1,
            operation,
            predecessor_state: t.predecessor_state,
            successor_state: t.successor_state,
            transition_digest: d(20),
            prepared_transition_binding_digest: t.binding_digest().unwrap(),
            projection_semantic_digest: if operation == 2 {
                kagemusha_ordinary_payment_body_digest_v1(
                    o.binding_digest().unwrap(),
                    o.encrypted_credit_digest,
                )
                .unwrap()
            } else {
                d(21)
            },
            lifecycle_binding_digest: t.lifecycle_digest,
            request_digest: t.request_digest,
            artifact_manifest_digest: if operation == 4 { d(22) } else { [0; 32] },
            preparation_guard_digest: d(23),
            reservation_digest: t.reservation_digest,
            preparation_authorization_digest: d(24),
            stream_lengths: [29, 31],
            stream_digests: [d(25), d(26)],
        }
    }
    fn intent(operation: u8) -> KagemushaOrdinaryCashTerminalIntentV1 {
        let p = prepared(operation);
        KagemushaOrdinaryCashTerminalIntentV1 {
            version: 1,
            operation,
            native_operation_id: d(27),
            native_nonce: d(28),
            preparation_id: p.binding_digest().unwrap(),
            candidate_digest: d(29),
            state_statement_digest: p.transition_digest,
            predecessor_descriptor_prefix_digest: d(30),
            sender_credential_digest: d(31),
            reservation_digest: p.reservation_digest,
            secure_index_before: (1_u128 << 100) + 41,
            secure_index_after: (1_u128 << 100) + 42,
            logical_journal_sequence_before: 10,
            logical_journal_sequence_after: 11,
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        }
    }
    fn body(operation: u8) -> KagemushaOrdinaryCashTerminalBodyV1 {
        let i = intent(operation);
        let p = prepared(operation);
        let o = output();
        KagemushaOrdinaryCashTerminalBodyV1 {
            version: 1,
            operation,
            amount: 17,
            state_statement_digest: i.state_statement_digest,
            candidate_digest: i.candidate_digest,
            preparation_id: i.preparation_id,
            prepared_projection_semantic_digest: p.projection_semantic_digest,
            lifecycle_digest: p.lifecycle_binding_digest,
            request_digest: p.request_digest,
            recipient_credential_digest: if operation == 2 { d(9) } else { [0; 32] },
            send_output_digest: if operation == 2 {
                o.binding_digest().unwrap()
            } else {
                [0; 32]
            },
            encrypted_credit_digest: if operation == 2 {
                o.encrypted_credit_digest
            } else {
                [0; 32]
            },
            artifact_manifest_digest: p.artifact_manifest_digest,
            reservation_digest: p.reservation_digest,
            native_operation_id: i.native_operation_id,
            terminal_intent_digest: i.binding_digest().unwrap(),
            predecessor_descriptor_prefix_digest: i.predecessor_descriptor_prefix_digest,
            stream_lengths: p.stream_lengths,
            stream_digests: p.stream_digests,
            clock_context: clock(),
            secure_index_before: i.secure_index_before,
            secure_index_after: i.secure_index_after,
            logical_journal_sequence_before: i.logical_journal_sequence_before,
            logical_journal_sequence_after: i.logical_journal_sequence_after,
        }
    }
    fn record(operation: u8) -> KagemushaOrdinaryCashTerminalRecordV1 {
        let i = intent(operation);
        let p = prepared(operation);
        let mut admitted = clock();
        admitted.lower_at_ms = 1003;
        admitted.upper_at_ms = 1004;
        KagemushaOrdinaryCashTerminalRecordV1 {
            version: 1,
            body: body(operation),
            sender_credential_digest: i.sender_credential_digest,
            preparation_authorization_digest: p.preparation_authorization_digest,
            terminal_authorization_digest: d(32),
            terminal_subject_digest: d(33),
            admission_clock_context: admitted,
            approval_issued_at_ms: i.issued_at_ms,
            approval_expires_at_ms: i.expires_at_ms,
        }
    }
    macro_rules! roundtrip {
        ($value:expr,$ty:ty) => {{
            let value = $value;
            let bytes = value.canonical_bytes().unwrap();
            assert_eq!(<$ty>::decode_canonical_exact(&bytes).unwrap(), value);
            assert_eq!(
                value.canonical_original_digest().unwrap(),
                original_digest(&bytes)
            );
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(<$ty>::decode_canonical_exact(&trailing).is_err());
            let transcript = value.binding_transcript();
            assert_eq!(
                value.binding_digest().unwrap(),
                <[u8; 32]>::from(Sha256::digest(&transcript.bytes))
            );
            assert_eq!(value.payload_transcript().bytes.len(), <$ty>::PAYLOAD_BYTES);
            let layout = value.original_preimage_layout().unwrap();
            let mut original = ORIGINAL_DOMAIN.to_vec();
            original.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            original.extend_from_slice(&bytes);
            assert_eq!(layout.bytes.len(), original.len());
            assert_eq!(&original[layout.original.clone()], bytes.as_slice());
            for (index, expected) in layout.bytes.iter().enumerate() {
                if let Some(expected) = expected {
                    assert_eq!(*expected, original[index]);
                }
            }
            assert!(
                layout
                    .fields
                    .iter()
                    .all(|field| !field.positions.is_empty())
            );
            assert!(
                layout
                    .fields
                    .iter()
                    .flat_map(|field| &field.positions)
                    .all(|position| layout.bytes[*position].is_none())
            );
        }};
    }
    #[test]
    fn ordinary_cash_stream_pairs_preserve_complete_json_and_canonical_binary() {
        fn roundtrip<T>(value: &T)
        where
            T: core::fmt::Debug
                + PartialEq
                + norito::NoritoSerialize
                + norito::json::JsonSerialize
                + norito::json::JsonDeserialize,
        {
            let binary = norito::encode_canonical(value).unwrap();
            let json = norito::json::to_json(value).unwrap();
            let decoded: T = norito::json::from_str(&json).unwrap();
            assert_eq!(&decoded, value);
            assert_eq!(norito::encode_canonical(&decoded).unwrap(), binary);
            assert_eq!(
                norito::json::to_json_bounded(value, json.len()).unwrap(),
                json,
            );
            assert!(norito::json::to_json_bounded(value, json.len() - 1).is_err());
            let pair = "\"stream_lengths\":[29,31]";
            assert_eq!(json.matches(pair).count(), 1);
            for changed in [
                "\"stream_lengths\":[]",
                "\"stream_lengths\":[29]",
                "\"stream_lengths\":[29,31,37]",
            ] {
                assert!(norito::json::from_str::<T>(&json.replace(pair, changed)).is_err());
            }
        }
        for operation in [2, 4] {
            roundtrip(&prepared(operation));
            roundtrip(&body(operation));
            roundtrip(&record(operation));
        }
    }

    #[test]
    fn ordinary_cash_all_fixed_records_roundtrip_and_model_owned_original_layouts() {
        roundtrip!(clock(), KagemushaOrdinaryCashClockContextV1);
        roundtrip!(request_body(), KagemushaOrdinaryPaymentRequestBodyV1);
        roundtrip!(output(), KagemushaOrdinaryPaymentOutputV1);
        for operation in [2, 4] {
            roundtrip!(transition(operation), KagemushaOrdinaryPreparedTransitionV1);
            roundtrip!(prepared(operation), KagemushaOrdinaryPreparedOutgoingV1);
            roundtrip!(intent(operation), KagemushaOrdinaryCashTerminalIntentV1);
            roundtrip!(body(operation), KagemushaOrdinaryCashTerminalBodyV1);
            roundtrip!(record(operation), KagemushaOrdinaryCashTerminalRecordV1);
            prepared(operation)
                .validate_against_transition(&transition(operation))
                .unwrap();
            body(operation)
                .validate_against_intent(&intent(operation))
                .unwrap();
            body(operation)
                .validate_against_prepared(&prepared(operation))
                .unwrap();
            record(operation)
                .validate_against_originals(&intent(operation), &prepared(operation))
                .unwrap();
        }
        output().validate_against_clock(&clock()).unwrap();
    }
    #[test]
    fn ordinary_receiver_request_genuine_platform_message_and_complete_original_roundtrip() {
        let (request, key) = request();
        let bytes = request.canonical_bytes().unwrap();
        assert_eq!(
            KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(&bytes).unwrap(),
            request
        );
        assert_eq!(
            request.canonical_original_digest().unwrap(),
            original_digest(&bytes)
        );
        let layout = request.original_preimage_layout().unwrap();
        assert!(
            layout
                .fields
                .iter()
                .any(|field| field.name == "evidence.signature_der")
        );
        let public = super::super::KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        request
            .evidence
            .authenticate_signature(
                KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                &public,
                d(40),
                d(41),
                None,
                &request.body.canonical_signing_bytes().unwrap(),
            )
            .unwrap();
        for index in 0..8 {
            let mut changed = request.clone();
            match index {
                0 => changed.body.amount += 1,
                1 => changed.body.recipient_account_binding[31] ^= 1,
                2 => changed.body.recipient_credential_digest[0] ^= 1,
                3 => {
                    changed
                        .body
                        .clock_context
                        .signed_observations_original_digest[31] ^= 1
                }
                4 => changed.body.clock_context.upper_at_ms += 1,
                5 => changed.body.recipient_lane_id[0] ^= 1,
                6 => changed.body.request_id[0] ^= 1,
                _ => changed.body.expires_at_ms += 1,
            }
            assert_ne!(
                changed.canonical_original_digest().unwrap(),
                request.canonical_original_digest().unwrap()
            );
            assert!(
                request
                    .evidence
                    .authenticate_signature(
                        KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                        &public,
                        d(40),
                        d(41),
                        None,
                        &changed.body.canonical_signing_bytes().unwrap()
                    )
                    .is_err()
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(&trailing).is_err());
        assert!(
            KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(&vec![
                0;
                KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
    }
    #[test]
    fn ordinary_preparation_transcripts_exact_existing_acyclic_fields_and_no_late_signature_input()
    {
        let t = transition(2);
        let mut expected = KAGEMUSHA_ORDINARY_PREPARED_TRANSITION_DOMAIN_V1.to_vec();
        expected.extend_from_slice(&1_u16.to_le_bytes());
        expected.push(2);
        for digest in [
            t.lifecycle_digest,
            t.request_digest,
            t.predecessor_state,
            t.successor_state,
        ] {
            expected.extend_from_slice(&digest);
        }
        expected.extend_from_slice(&t.amount.to_le_bytes());
        expected.extend_from_slice(&t.reservation_digest);
        expected.extend_from_slice(&t.native_preparation_operation_id);
        assert_eq!(t.binding_transcript().bytes, expected);
        assert_eq!(expected.len(), 259);
        let p = prepared(2);
        let mut changed = p;
        changed.preparation_authorization_digest[0] ^= 1;
        assert_ne!(
            p.binding_digest().unwrap(),
            changed.binding_digest().unwrap()
        );
        assert_eq!(p.binding_transcript().bytes.len(), 484);
        assert_eq!(
            t.binding_digest().unwrap(),
            transition(2).binding_digest().unwrap()
        );
        let mut r = record(2);
        let before = r.body.binding_digest().unwrap();
        r.terminal_authorization_digest[0] ^= 1;
        assert_eq!(r.body.binding_digest().unwrap(), before);
        assert_ne!(
            r.binding_digest().unwrap(),
            record(2).binding_digest().unwrap()
        );
    }
    #[test]
    fn ordinary_cash_original_intervals_independent_indexes_and_slot_mutations_rejected() {
        let mut clock_context = clock();
        clock_context.upper_at_ms = 2000;
        assert!(
            clock_context
                .validate_within_original_window(1000, 2000)
                .is_err()
        );
        let mut clock_context = clock();
        clock_context.lower_at_ms = 0;
        assert!(clock_context.binding_digest().is_err());
        let mut clock_context = clock();
        clock_context.lower_at_ms = clock_context.upper_at_ms + 1;
        assert!(clock_context.binding_digest().is_err());
        let mut request_record = request_body();
        request_record.clock_context.upper_at_ms = request_record.expires_at_ms;
        assert!(request_record.canonical_signing_bytes().is_err());
        let mut payment_output = output();
        payment_output.prepared_at_ms += 1;
        assert!(payment_output.validate_against_clock(&clock()).is_err());
        let mut payment_output = output();
        payment_output.credit_id[0] ^= 1;
        assert!(payment_output.binding_digest().is_err());
        for operation in [2, 4] {
            let mut terminal_intent = intent(operation);
            terminal_intent.secure_index_after =
                u128::from(terminal_intent.logical_journal_sequence_after);
            assert!(terminal_intent.binding_digest().is_err());
            let mut terminal_intent = intent(operation);
            terminal_intent.native_nonce = [0; 32];
            assert!(terminal_intent.binding_digest().is_err());
            let mut terminal_intent = intent(operation);
            terminal_intent.expires_at_ms = terminal_intent.issued_at_ms
                + super::super::KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1
                + 1;
            assert!(terminal_intent.binding_digest().is_err());
            let mut terminal_body = body(operation);
            terminal_body.stream_lengths[0] =
                u64::from(KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1) + 1;
            assert!(terminal_body.binding_digest().is_err());
            let mut prepared_outgoing = prepared(operation);
            prepared_outgoing.request_digest = if operation == 2 { [0; 32] } else { d(40) };
            assert!(prepared_outgoing.binding_digest().is_err());
            let mut prepared_outgoing = prepared(operation);
            prepared_outgoing.preparation_authorization_digest[31] ^= 1;
            assert!(
                record(operation)
                    .validate_against_originals(&intent(operation), &prepared_outgoing)
                    .is_err()
            );
            let mut terminal_body = body(operation);
            terminal_body.amount += 1;
            assert_ne!(
                terminal_body.binding_digest().unwrap(),
                body(operation).binding_digest().unwrap()
            );
            let mut terminal_body = body(operation);
            terminal_body.native_operation_id[0] ^= 1;
            assert!(
                terminal_body
                    .validate_against_intent(&intent(operation))
                    .is_err()
            );
            let mut terminal_body = body(operation);
            terminal_body.secure_index_before += 1;
            terminal_body.secure_index_after += 1;
            assert!(
                terminal_body
                    .validate_against_intent(&intent(operation))
                    .is_err()
            );
            let mut request_record = record(operation);
            request_record.admission_clock_context.upper_at_ms =
                request_record.approval_expires_at_ms;
            assert!(request_record.binding_digest().is_err());
            let mut request_record = record(operation);
            request_record.sender_credential_digest[0] ^= 1;
            assert!(
                request_record
                    .validate_against_originals(&intent(operation), &prepared(operation))
                    .is_err()
            );
            let mut request_record = record(operation);
            request_record.approval_expires_at_ms += 1;
            assert!(
                request_record
                    .validate_against_originals(&intent(operation), &prepared(operation))
                    .is_err()
            );
        }
        assert!(kagemusha_ordinary_payment_body_digest_v1([0; 32], d(1)).is_err());
        assert!(kagemusha_ordinary_output_binding_digest_v1(d(1), d(2), [0; 32]).is_err());
        assert_ne!(
            kagemusha_ordinary_output_binding_digest_v1(d(1), d(2), d(3)).unwrap(),
            kagemusha_ordinary_output_binding_digest_v1(d(1), d(2), d(4)).unwrap()
        );
    }
}

/// Bind the exact body/candidate/State/reservation before purpose1 W signs normalized Guard.
/// This data formula deliberately excludes W1 and final output binding to avoid a hash cycle.
/// It authenticates no body, candidate, financial reservation or approval.
/// # Errors
/// Rejects any absent body, candidate, complete State SHA or financial reservation selector.
pub fn kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
    body_digest: [u8; 32],
    candidate_digest: [u8; 32],
    state_statement_digest: [u8; 32],
    reservation_digest: [u8; 32],
) -> Result<[u8; 32], String> {
    nonzero(&[
        body_digest,
        candidate_digest,
        state_statement_digest,
        reservation_digest,
    ])?;
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_TERMINAL_GUARD_COMMIT_DOMAIN_V1);
    for original in [
        body_digest,
        candidate_digest,
        state_statement_digest,
        reservation_digest,
    ] {
        hash.update(original);
    }
    Ok(hash.finalize().into())
}
/// Derive the ordinary predecessor conflict nullifier from actual selected financial State.
/// Financial secure index is the complete unsigned128 value; logical sequence/journal/Apple
/// counter is never substituted or cast. This formula creates neither a reservation nor money.
/// # Errors
/// Rejects any absent State/epoch/network/lane/pool selector.
pub fn kagemusha_ordinary_transition_nullifier_v1(
    predecessor_state: [u8; 32],
    predecessor_secure_index: u128,
    predecessor_epoch: [u8; 32],
    network_id: [u8; 32],
    lane_id: [u8; 32],
    reserve_pool_id: [u8; 32],
) -> Result<[u8; 32], String> {
    nonzero(&[
        predecessor_state,
        predecessor_epoch,
        network_id,
        lane_id,
        reserve_pool_id,
    ])?;
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_TRANSITION_NULLIFIER_DOMAIN_V1);
    hash.update(predecessor_state);
    hash.update(predecessor_secure_index.to_le_bytes());
    for original in [predecessor_epoch, network_id, lane_id, reserve_pool_id] {
        hash.update(original);
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod terminal_formula_tests {
    use super::*;
    #[test]
    fn ordinary_terminal_guard_commit_is_exact_pre_approval_four_digest_transcript() {
        let fields = [[1; 32], [2; 32], [3; 32], [4; 32]];
        let mut expected = KAGEMUSHA_ORDINARY_TERMINAL_GUARD_COMMIT_DOMAIN_V1.to_vec();
        for field in fields {
            expected.extend_from_slice(&field);
        }
        let actual = kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            fields[0], fields[1], fields[2], fields[3],
        )
        .unwrap();
        assert_eq!(actual, <[u8; 32]>::from(Sha256::digest(expected)));
        for index in 0..4 {
            let mut changed = fields;
            changed[index] = [0; 32];
            assert!(
                kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
                    changed[0], changed[1], changed[2], changed[3]
                )
                .is_err()
            );
        }
        assert_ne!(
            actual,
            kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
                fields[1], fields[0], fields[2], fields[3]
            )
            .unwrap()
        );
    }
    #[test]
    fn ordinary_transition_nullifier_preserves_full_secure_index_and_exact_field_order() {
        let index = (1_u128 << 100) + 41;
        let fields = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]];
        let mut expected = KAGEMUSHA_ORDINARY_TRANSITION_NULLIFIER_DOMAIN_V1.to_vec();
        expected.extend_from_slice(&fields[0]);
        expected.extend_from_slice(&index.to_le_bytes());
        for field in &fields[1..] {
            expected.extend_from_slice(field);
        }
        let actual = kagemusha_ordinary_transition_nullifier_v1(
            fields[0], index, fields[1], fields[2], fields[3], fields[4],
        )
        .unwrap();
        assert_eq!(actual, <[u8; 32]>::from(Sha256::digest(expected)));
        assert_ne!(
            actual,
            kagemusha_ordinary_transition_nullifier_v1(
                fields[0], 41, fields[1], fields[2], fields[3], fields[4]
            )
            .unwrap()
        );
        for slot in 0..5 {
            let mut changed = fields;
            changed[slot] = [0; 32];
            assert!(
                kagemusha_ordinary_transition_nullifier_v1(
                    changed[0], index, changed[1], changed[2], changed[3], changed[4]
                )
                .is_err()
            );
        }
    }

    #[test]
    fn maintained_sealed_stream_commitments_cover_domains_lengths_and_complete_originals() {
        type StreamDigest = fn(&[u8]) -> Result<[u8; 32], String>;

        let transition = KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize;
        let recovery = KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize;
        let cases: [(&[u8], usize, StreamDigest); 2] = [
            (
                KAGEMUSHA_ORDINARY_SEALED_TRANSITION_INPUTS_DOMAIN_V1,
                transition,
                kagemusha_ordinary_sealed_transition_inputs_digest_v1,
            ),
            (
                KAGEMUSHA_ORDINARY_SEALED_RECOVERY_SEEDS_DOMAIN_V1,
                recovery,
                kagemusha_ordinary_sealed_recovery_seeds_digest_v1,
            ),
        ];
        for (domain, maximum, digest) in cases {
            assert_eq!(domain.last(), Some(&0));
            for length in [1, 7, 15, 16, 55, 56, 63, 64, 65, maximum] {
                let raw: Vec<_> = (0..length)
                    .map(|i| {
                        u8::try_from(i % 256)
                            .unwrap()
                            .wrapping_mul(37)
                            .wrapping_add(11)
                    })
                    .collect();
                let mut exact = domain.to_vec();
                exact.extend((length as u64).to_le_bytes());
                exact.extend(&raw);
                let expected: [u8; 32] = Sha256::digest(exact).into();
                assert_eq!(digest(&raw).unwrap(), expected);
                assert_ne!(expected, <[u8; 32]>::from(Sha256::digest(&raw)));
                let mut changed = raw.clone();
                changed[length - 1] ^= 1;
                assert_ne!(digest(&changed).unwrap(), expected);
                changed.push(0);
                if changed.len() <= maximum {
                    assert_ne!(digest(&changed).unwrap(), digest(&raw).unwrap());
                }
            }
        }
        assert_ne!(
            kagemusha_ordinary_sealed_transition_inputs_digest_v1(&[1]).unwrap(),
            kagemusha_ordinary_sealed_recovery_seeds_digest_v1(&[1]).unwrap()
        );
    }
    #[test]
    fn maintained_sealed_stream_commitments_refuse_empty_and_oversized_originals() {
        assert!(kagemusha_ordinary_sealed_transition_inputs_digest_v1(&[]).is_err());
        assert!(kagemusha_ordinary_sealed_recovery_seeds_digest_v1(&[]).is_err());
        assert!(
            kagemusha_ordinary_sealed_transition_inputs_digest_v1(&vec![
                0;
                KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1
                    as usize
                    + 1
            ])
            .is_err()
        );
        assert!(
            kagemusha_ordinary_sealed_recovery_seeds_digest_v1(&vec![
                0;
                KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1
                    as usize
                    + 1
            ])
            .is_err()
        );
    }
}

#[path = "ordinary_payment_request_stream.rs"]
mod request_stream;
pub use request_stream::{
    KagemushaOrdinaryPaymentRequestStreamGrammarV1, KagemushaOrdinaryPaymentRequestStreamVariantV1,
};
