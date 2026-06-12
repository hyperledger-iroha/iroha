//! Gateway proof token helpers for the SoraFS moderation pipeline.
//!
//! The types below encode the `ProofTokenV1` structure from the SoraFS gateway
//! compliance plan and provide deterministic helpers for minting and verifying
//! response headers (`Sora-Moderation-Token`).

use std::{
    convert::TryFrom as _,
    string::String,
    time::{Duration, SystemTime, UNIX_EPOCH},
    vec::Vec,
};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use blake3::Hasher;
use ed25519_dalek::{SIGNATURE_LENGTH, Signature, Signer, SigningKey, VerifyingKey};
use rand_core::TryCryptoRng;
use thiserror::Error;
use zeroize::Zeroizing;

const FRAME_MAGIC: &[u8; 4] = b"SFGT";
const DIGEST_DOMAIN: &[u8] = b"sorafs.proof_token.digest.v1";
const SIGNING_DOMAIN: &[u8] = b"sorafs.proof_token.sign.v1";
const MAX_ENTRY_IDS: usize = 32;
const MAX_ENTRY_LEN: usize = 255;
const FLAG_HAS_EXPIRY: u8 = 0x01;

/// Secret used to derive the blinded digest portion of a token body.
#[derive(Clone)]
pub struct ProofTokenDigestKey(Zeroizing<[u8; 32]>);

impl ProofTokenDigestKey {
    /// Construct a new digest key from raw bytes.
    #[must_use]
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    #[must_use]
    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Moderation action classification embedded inside the token body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationAction {
    /// Content was blocked immediately.
    Block,
    /// Content was quarantined pending review.
    Quarantine,
    /// The caller was rate limited.
    RateLimit,
    /// A warning or redirect was issued.
    Redirect,
    /// Reserved/custom action code.
    Custom(u8),
}

impl ModerationAction {
    #[must_use]
    fn to_u8(self) -> u8 {
        match self {
            Self::Block => 0,
            Self::Quarantine => 1,
            Self::RateLimit => 2,
            Self::Redirect => 3,
            Self::Custom(code) => code,
        }
    }

    #[must_use]
    fn from_u8(code: u8) -> Self {
        match code {
            0 => Self::Block,
            1 => Self::Quarantine,
            2 => Self::RateLimit,
            3 => Self::Redirect,
            other => Self::Custom(other),
        }
    }
}

/// Minting parameters for a [`ProofToken`].
pub struct ProofTokenParams<'a> {
    /// Moderation action classification to encode.
    pub moderation: ModerationAction,
    /// Denylist entry identifiers tied to the decision.
    pub entry_ids: &'a [&'a str],
    /// Digest of the evidence bundle referenced by the token.
    pub evidence_digest: &'a [u8; 32],
    /// Timestamp when the token becomes valid.
    pub issued_at: SystemTime,
    /// Optional expiry timestamp (rate limiting or warnings).
    pub expires_at: Option<SystemTime>,
}

/// Proof token issued for every gateway moderation action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofToken {
    token_id: [u8; 16],
    moderation: ModerationAction,
    issued_at: u64,
    expires_at: Option<u64>,
    entry_ids: Vec<String>,
    blinded_digest: [u8; 32],
    signature: Signature,
}

struct FrameReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> FrameReader<'a> {
    fn new(bytes: &'a [u8], cursor: usize) -> Self {
        Self { bytes, cursor }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.cursor.checked_add(len).ok_or(DecodeError::Truncated)?;
        if end > self.bytes.len() {
            return Err(DecodeError::Truncated);
        }
        let slice = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(slice)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.cursor)
    }
}

impl ProofToken {
    /// Current on-wire version.
    pub const VERSION: u8 = 1;

    /// Mint a new proof token.
    #[allow(clippy::missing_errors_doc)]
    pub fn mint<R: TryCryptoRng>(
        rng: &mut R,
        digest_key: &ProofTokenDigestKey,
        signing_key: &SigningKey,
        params: &ProofTokenParams<'_>,
    ) -> Result<Self, MintError> {
        if params.entry_ids.is_empty() {
            return Err(MintError::MissingEntries);
        }
        if params.entry_ids.len() > MAX_ENTRY_IDS {
            return Err(MintError::TooManyEntries {
                max: MAX_ENTRY_IDS,
                actual: params.entry_ids.len(),
            });
        }

        let issued_at = to_unix_seconds(params.issued_at)?;
        let expires_at = match params.expires_at {
            Some(ts) => {
                let secs = to_unix_seconds(ts)?;
                if secs <= issued_at {
                    return Err(MintError::InvalidExpiry);
                }
                Some(secs)
            }
            None => None,
        };

        let mut token_id = [0u8; 16];
        fill_random(rng, "minting proof token id", &mut token_id)?;

        let mut entry_ids: Vec<String> = Vec::with_capacity(params.entry_ids.len());
        for &entry in params.entry_ids {
            if entry.is_empty() {
                return Err(MintError::EmptyEntryId);
            }
            if entry.len() > MAX_ENTRY_LEN {
                return Err(MintError::EntryTooLong {
                    max: MAX_ENTRY_LEN,
                    actual: entry.len(),
                });
            }
            entry_ids.push(entry.to_string());
        }

        let blinded_digest =
            compute_blinded_digest(digest_key, &token_id, params.evidence_digest, &entry_ids)
                .map_err(MintError::Encoding)?;

        let mut token = Self {
            token_id,
            moderation: params.moderation,
            issued_at,
            expires_at,
            entry_ids,
            blinded_digest,
            signature: Signature::from_bytes(&[0; SIGNATURE_LENGTH]),
        };
        let body = token
            .body_without_signature()
            .map_err(MintError::Encoding)?;
        let message = signing_message(&body);
        token.signature = signing_key.sign(&message);
        Ok(token)
    }

    /// Try to serialize the token frame.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] when a directly constructed token contains entry
    /// counts or fields that cannot be represented by the fixed-width v1 frame.
    pub fn try_encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut body = self.body_without_signature()?;
        let sig_bytes = self.signature.to_bytes();
        let sig_len =
            u16::try_from(sig_bytes.len()).map_err(|_| EncodeError::SignatureTooLong {
                max: usize::from(u16::MAX),
                actual: sig_bytes.len(),
            })?;
        body.extend_from_slice(&sig_len.to_be_bytes());
        body.extend_from_slice(&sig_bytes);

        let mut out = Vec::with_capacity(FRAME_MAGIC.len() + body.len());
        out.extend_from_slice(FRAME_MAGIC);
        out.extend_from_slice(&body);
        Ok(out)
    }

    /// Serialize the token frame.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.try_encode().unwrap_or_else(|_| FRAME_MAGIC.to_vec())
    }

    /// Serialize the token as URL-safe base64 (header-friendly).
    #[must_use]
    pub fn encode_base64(&self) -> String {
        encode_base64_url_no_pad(&self.encode())
    }

    /// Decode a token from its binary frame.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] when the payload is truncated, malformed, or
    /// uses an unsupported version. Also enforces mint invariants such as
    /// canonical flags, non-empty entry lists, and `expires_at` strictly after
    /// `issued_at`.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < FRAME_MAGIC.len() + 2 {
            return Err(DecodeError::Truncated);
        }
        if &bytes[..FRAME_MAGIC.len()] != FRAME_MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let mut reader = FrameReader::new(bytes, FRAME_MAGIC.len());

        let version = reader.take_array::<1>()?[0];
        if version != Self::VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }

        let flags = reader.take_array::<1>()?[0];
        if flags & !FLAG_HAS_EXPIRY != 0 {
            return Err(DecodeError::InvalidFlags(flags));
        }
        let moderation = ModerationAction::from_u8(reader.take_array::<1>()?[0]);
        let issued_at = u64::from_be_bytes(reader.take_array::<8>()?);
        let expires_at = if flags & FLAG_HAS_EXPIRY == FLAG_HAS_EXPIRY {
            Some(u64::from_be_bytes(reader.take_array::<8>()?))
        } else {
            None
        };
        if let Some(expires_at) = expires_at
            && expires_at <= issued_at
        {
            return Err(DecodeError::InvalidExpiry {
                issued_at,
                expires_at,
            });
        }
        if unix_time_from_secs(issued_at).is_none() {
            return Err(DecodeError::TimestampOutOfRange {
                field: "issued_at",
                value: issued_at,
            });
        }
        if let Some(expires_at) = expires_at
            && unix_time_from_secs(expires_at).is_none()
        {
            return Err(DecodeError::TimestampOutOfRange {
                field: "expires_at",
                value: expires_at,
            });
        }

        let token_id = reader.take_array::<16>()?;

        let entry_count = u16::from_be_bytes(reader.take_array::<2>()?) as usize;
        if entry_count == 0 {
            return Err(DecodeError::MissingEntries);
        }
        if entry_count > MAX_ENTRY_IDS {
            return Err(DecodeError::TooManyEntries(entry_count));
        }
        let mut entry_ids = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let len = u16::from_be_bytes(reader.take_array::<2>()?) as usize;
            if len == 0 || len > MAX_ENTRY_LEN {
                return Err(DecodeError::InvalidEntryLength(len));
            }
            let entry = reader.take(len)?;
            let entry = std::str::from_utf8(entry).map_err(|_| DecodeError::InvalidUtf8)?;
            entry_ids.push(entry.to_owned());
        }

        let blinded_digest = reader.take_array::<32>()?;

        let sig_len = u16::from_be_bytes(reader.take_array::<2>()?) as usize;
        let remaining = reader.remaining();
        if sig_len != remaining {
            return Err(DecodeError::InvalidSignatureLength {
                expected: sig_len,
                actual: remaining,
            });
        }
        let sig_slice = reader.take(sig_len)?;
        let sig_bytes: [u8; SIGNATURE_LENGTH] =
            sig_slice
                .try_into()
                .map_err(|_| DecodeError::InvalidSignatureLength {
                    expected: SIGNATURE_LENGTH,
                    actual: sig_slice.len(),
                })?;
        if signature_bytes_are_all_zero(&sig_bytes) {
            return Err(DecodeError::InertSignature);
        }
        let signature = Signature::from_bytes(&sig_bytes);

        Ok(Self {
            token_id,
            moderation,
            issued_at,
            expires_at,
            entry_ids,
            blinded_digest,
            signature,
        })
    }

    /// Decode a token from its base64 representation.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::Base64`] if the text is not valid base64 or any
    /// [`DecodeError`] emitted by [`ProofToken::decode`].
    pub fn decode_base64(s: &str) -> Result<Self, DecodeError> {
        let decoded = decode_base64_url_no_pad(s)?;
        Self::decode(&decoded)
    }

    /// Return the moderation action classification.
    #[must_use]
    pub fn moderation(&self) -> ModerationAction {
        self.moderation
    }

    /// UNIX timestamp (seconds) describing when the token was issued.
    #[must_use]
    pub fn issued_at(&self) -> SystemTime {
        self.checked_issued_at().unwrap_or(UNIX_EPOCH)
    }

    /// UNIX timestamp (seconds) describing when the token was issued, if it is
    /// representable by `SystemTime`.
    #[must_use]
    pub fn checked_issued_at(&self) -> Option<SystemTime> {
        unix_time_from_secs(self.issued_at)
    }

    /// Optional expiry timestamp.
    #[must_use]
    pub fn expires_at(&self) -> Option<SystemTime> {
        self.expires_at
            .map(|ts| unix_time_from_secs(ts).unwrap_or(UNIX_EPOCH))
    }

    /// Optional expiry timestamp, if present and representable by `SystemTime`.
    #[must_use]
    pub fn checked_expires_at(&self) -> Option<SystemTime> {
        self.expires_at.and_then(unix_time_from_secs)
    }

    /// Token identifier bytes (UUID-compatible).
    #[must_use]
    pub fn token_id(&self) -> [u8; 16] {
        self.token_id
    }

    /// Borrow the entry identifiers encoded in the token.
    #[must_use]
    pub fn entry_ids(&self) -> &[String] {
        &self.entry_ids
    }

    /// Access the blinded digest that commits to the moderation evidence.
    #[must_use]
    pub fn blinded_digest(&self) -> &[u8; 32] {
        &self.blinded_digest
    }

    /// Verify the detached Ed25519 signature covering the token body.
    ///
    /// # Errors
    ///
    /// Returns [`VerificationError::InvalidSignature`] when verification fails. Uses strict
    /// Ed25519 verification and rejects weak public keys and non-canonical signatures.
    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> Result<(), VerificationError> {
        if verifying_key.is_weak() {
            return Err(VerificationError::InvalidSignature);
        }
        if signature_bytes_are_all_zero(&self.signature.to_bytes()) {
            return Err(VerificationError::InertSignature);
        }
        let body = self
            .body_without_signature()
            .map_err(|_| VerificationError::InvalidSignature)?;
        let message = signing_message(&body);
        verifying_key
            .verify_strict(&message, &self.signature)
            .map_err(|_| VerificationError::InvalidSignature)
    }

    /// Recompute the blinded digest using the shared secret and evidence hash.
    ///
    /// # Errors
    ///
    /// Returns [`VerificationError::BlindedDigestMismatch`] when the digest
    /// does not match the supplied evidence hash.
    pub fn verify_blinded_digest(
        &self,
        digest_key: &ProofTokenDigestKey,
        evidence_digest: &[u8; 32],
    ) -> Result<(), VerificationError> {
        let expected =
            compute_blinded_digest(digest_key, &self.token_id, evidence_digest, &self.entry_ids)
                .map_err(|_| VerificationError::BlindedDigestMismatch)?;
        if expected == self.blinded_digest {
            Ok(())
        } else {
            Err(VerificationError::BlindedDigestMismatch)
        }
    }

    fn body_without_signature(&self) -> Result<Vec<u8>, EncodeError> {
        let mut out = Vec::new();
        out.push(Self::VERSION);
        let mut flags = 0u8;
        if self.expires_at.is_some() {
            flags |= FLAG_HAS_EXPIRY;
        }
        out.push(flags);
        out.push(self.moderation.to_u8());
        out.extend_from_slice(&self.issued_at.to_be_bytes());
        if let Some(ts) = self.expires_at {
            out.extend_from_slice(&ts.to_be_bytes());
        }
        out.extend_from_slice(&self.token_id);
        let entry_count =
            u16::try_from(self.entry_ids.len()).map_err(|_| EncodeError::EntryCountTooLarge {
                max: usize::from(u16::MAX),
                actual: self.entry_ids.len(),
            })?;
        out.extend_from_slice(&entry_count.to_be_bytes());
        for entry in &self.entry_ids {
            let entry_bytes = entry.as_bytes();
            let len = u16::try_from(entry_bytes.len()).map_err(|_| EncodeError::EntryTooLong {
                max: usize::from(u16::MAX),
                actual: entry_bytes.len(),
            })?;
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(entry_bytes);
        }
        out.extend_from_slice(&self.blinded_digest);
        Ok(out)
    }
}

/// Errors surfaced while serializing proof tokens.
#[derive(Debug, Clone, Copy, Error)]
pub enum EncodeError {
    /// More entries were present than the v1 frame can encode.
    #[error("too many entry ids to encode: max {max}, got {actual}")]
    EntryCountTooLarge {
        /// Maximum number of entries encodable by the v1 frame.
        max: usize,
        /// Actual entry count observed.
        actual: usize,
    },
    /// An entry identifier exceeded the v1 length prefix range.
    #[error("entry id too long to encode: max {max} bytes, got {actual}")]
    EntryTooLong {
        /// Maximum size encodable by the v1 frame.
        max: usize,
        /// Actual entry size observed.
        actual: usize,
    },
    /// Signature bytes exceeded the v1 length prefix range.
    #[error("signature too long to encode: max {max} bytes, got {actual}")]
    SignatureTooLong {
        /// Maximum signature size encodable by the v1 frame.
        max: usize,
        /// Actual signature size observed.
        actual: usize,
    },
}

/// Errors surfaced when minting new tokens.
#[derive(Debug, Clone, Error)]
pub enum MintError {
    /// Caller attempted to mint a token without any denylist entries.
    #[error("at least one entry id is required")]
    MissingEntries,
    /// Too many entries were supplied for a single token.
    #[error("too many entry ids: max {max}, got {actual}")]
    TooManyEntries {
        /// Configured maximum number of entries.
        max: usize,
        /// Actual entry count supplied by the caller.
        actual: usize,
    },
    /// Entry identifiers must not be empty.
    #[error("entry ids must not be empty")]
    EmptyEntryId,
    /// An entry identifier exceeded the maximum allowed size.
    #[error("entry id too long: max {max} bytes, got {actual}")]
    EntryTooLong {
        /// Maximum size permitted for a single entry id.
        max: usize,
        /// Actual size that triggered the error.
        actual: usize,
    },
    /// `issued_at` or `expires_at` could not be converted to UNIX seconds.
    #[error("timestamp out of range for unix epoch")]
    TimestampOutOfRange,
    /// `expires_at` was equal to or earlier than `issued_at`.
    #[error("expires_at must be strictly greater than issued_at")]
    InvalidExpiry,
    /// Random byte generation failed while minting the token.
    #[error("random byte generation failed while {operation}: {message}")]
    RandomBytes {
        /// Operation that requested random bytes.
        operation: &'static str,
        /// Underlying RNG error message.
        message: String,
    },
    /// Token body could not be represented in the v1 frame.
    #[error("proof token body encoding failed: {0}")]
    Encoding(EncodeError),
}

/// Errors surfaced while decoding proof tokens.
#[derive(Debug, Clone, Copy, Error)]
pub enum DecodeError {
    /// Frame contained fewer bytes than required.
    #[error("token truncated")]
    Truncated,
    /// Encountered a future or unsupported token version.
    #[error("unsupported proof token version {0}")]
    UnsupportedVersion(u8),
    /// Frame did not begin with the expected `SFGT` magic.
    #[error("invalid frame magic")]
    BadMagic,
    /// Frame flags contain unsupported bits.
    #[error("invalid token flags {0:#04x}")]
    InvalidFlags(u8),
    /// Entry list is empty.
    #[error("entry list must not be empty")]
    MissingEntries,
    /// More entry ids were present than the helper supports.
    #[error("too many entry ids ({0})")]
    TooManyEntries(usize),
    /// Entry identifier length was invalid or would overflow.
    #[error("invalid entry length {0}")]
    InvalidEntryLength(usize),
    /// Entry identifier bytes were not valid UTF-8.
    #[error("entry id contains invalid utf-8")]
    InvalidUtf8,
    /// `expires_at` timestamp was not after `issued_at`.
    #[error("expires_at {expires_at} must be greater than issued_at {issued_at}")]
    InvalidExpiry {
        /// Issued-at timestamp (UNIX seconds).
        issued_at: u64,
        /// Expiry timestamp (UNIX seconds).
        expires_at: u64,
    },
    /// Timestamp could not be represented as `SystemTime`.
    #[error("{field} timestamp {value} is out of range for system time")]
    TimestampOutOfRange {
        /// Timestamp field name.
        field: &'static str,
        /// UNIX-second timestamp carried by the frame.
        value: u64,
    },
    /// Signature length prefix did not match the trailing bytes.
    #[error("signature length mismatch (expected {expected}, actual {actual})")]
    InvalidSignatureLength {
        /// Signature size encoded inside the frame.
        expected: usize,
        /// Actual bytes remaining in the frame.
        actual: usize,
    },
    /// Signature bytes were an inert all-zero placeholder.
    #[error("proof token signature material must not be all zero")]
    InertSignature,
    /// Base64 payload was malformed.
    #[error("invalid base64 payload")]
    Base64,
}

/// Errors produced during verification.
#[derive(Debug, Clone, Copy, Error)]
pub enum VerificationError {
    /// Ed25519 signature did not verify with the supplied key.
    #[error("invalid proof token signature")]
    InvalidSignature,
    /// Signature bytes were an inert all-zero placeholder.
    #[error("proof token signature material must not be all zero")]
    InertSignature,
    /// Secret re-computed digest did not match the token body.
    #[error("blinded digest mismatch")]
    BlindedDigestMismatch,
}

fn to_unix_seconds(time: SystemTime) -> Result<u64, MintError> {
    time.duration_since(UNIX_EPOCH)
        .map_err(|_| MintError::TimestampOutOfRange)
        .map(|duration| duration.as_secs())
}

fn unix_time_from_secs(secs: u64) -> Option<SystemTime> {
    UNIX_EPOCH.checked_add(Duration::from_secs(secs))
}

fn fill_random<R: TryCryptoRng>(
    rng: &mut R,
    operation: &'static str,
    dest: &mut [u8],
) -> Result<(), MintError> {
    rng.try_fill_bytes(dest)
        .map_err(|err| MintError::RandomBytes {
            operation,
            message: err.to_string(),
        })?;
    if dest.iter().all(|&byte| byte == 0) {
        return Err(MintError::RandomBytes {
            operation,
            message: "rng returned all-zero material".to_owned(),
        });
    }
    Ok(())
}

fn encode_base64_url_no_pad(bytes: &[u8]) -> String {
    let encoded_len =
        base64::encoded_len(bytes.len(), false).expect("proof token base64 length fits usize");
    let mut buffer = vec![0u8; encoded_len];
    let written = base64::Engine::encode_slice(&URL_SAFE_NO_PAD, bytes, &mut buffer)
        .expect("proof token base64 buffer is pre-sized");
    buffer.truncate(written);
    buffer.into_iter().map(char::from).collect()
}

fn decode_base64_url_no_pad(s: &str) -> Result<Vec<u8>, DecodeError> {
    let mut buffer = vec![0u8; base64::decoded_len_estimate(s.len())];
    let written = base64::Engine::decode_slice(&URL_SAFE_NO_PAD, s, &mut buffer)
        .map_err(|_| DecodeError::Base64)?;
    buffer.truncate(written);
    Ok(buffer)
}

fn compute_blinded_digest(
    digest_key: &ProofTokenDigestKey,
    token_id: &[u8; 16],
    evidence_digest: &[u8; 32],
    entries: &[String],
) -> Result<[u8; 32], EncodeError> {
    let mut hasher = Hasher::new_keyed(digest_key.as_bytes());
    hasher.update(DIGEST_DOMAIN);
    hasher.update(token_id);
    hasher.update(evidence_digest);
    for entry in entries {
        let entry_bytes = entry.as_bytes();
        let len = u16::try_from(entry_bytes.len()).map_err(|_| EncodeError::EntryTooLong {
            max: usize::from(u16::MAX),
            actual: entry_bytes.len(),
        })?;
        hasher.update(&len.to_be_bytes());
        hasher.update(entry_bytes);
    }
    Ok(hasher.finalize().into())
}

fn signing_message(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(SIGNING_DOMAIN.len() + body.len());
    out.extend_from_slice(SIGNING_DOMAIN);
    out.extend_from_slice(body);
    out
}

fn signature_bytes_are_all_zero(signature: &[u8; SIGNATURE_LENGTH]) -> bool {
    signature.iter().all(|&byte| byte == 0)
}

#[cfg(test)]
mod tests {
    use curve25519_dalek::{
        edwards::EdwardsPoint,
        traits::{Identity, IsIdentity},
    };
    use ed25519_dalek::SigningKey;
    use ed25519_dalek::Verifier as _;
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;
    use rand_core::{TryCryptoRng, TryRngCore};
    use sha2::{Digest, Sha512};

    use super::*;

    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    struct FailingTryRng;

    #[derive(Debug)]
    struct FailingTryRngError;

    impl std::fmt::Display for FailingTryRngError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("failing proof token RNG")
        }
    }

    impl TryRngCore for FailingTryRng {
        type Error = FailingTryRngError;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(FailingTryRngError)
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(FailingTryRngError)
        }

        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
            Err(FailingTryRngError)
        }
    }

    impl TryCryptoRng for FailingTryRng {}

    struct FixedTryRng {
        byte: u8,
    }

    impl TryRngCore for FixedTryRng {
        type Error = core::convert::Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok(u32::from_le_bytes([self.byte; 4]))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(u64::from_le_bytes([self.byte; 8]))
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
            dest.fill(self.byte);
            Ok(())
        }
    }

    impl TryCryptoRng for FixedTryRng {}

    #[test]
    fn mint_roundtrip() {
        let mut rng = ChaCha20Rng::seed_from_u64(42);
        let digest_key = ProofTokenDigestKey::new([3; 32]);
        let signing = test_signing_key();
        let verifying = signing.verifying_key();
        let evidence = [9u8; 32];
        let params = ProofTokenParams {
            moderation: ModerationAction::Block,
            entry_ids: &["denylist/global", "manual/guardian"],
            evidence_digest: &evidence,
            issued_at: UNIX_EPOCH + Duration::from_secs(1_714_000_000),
            expires_at: None,
        };
        let token = ProofToken::mint(&mut rng, &digest_key, &signing, &params).unwrap();
        let encoded = token.encode();
        let decoded = ProofToken::decode(&encoded).unwrap();
        assert_eq!(token, decoded);
        decoded.verify_signature(&verifying).unwrap();
        decoded
            .verify_blinded_digest(&digest_key, &evidence)
            .unwrap();
    }

    #[test]
    fn digest_key_clone_preserves_blinded_digest() {
        let digest_key = ProofTokenDigestKey::new([0x13; 32]);
        let cloned = digest_key.clone();
        let token_id = [0x24; 16];
        let evidence = [0x42; 32];
        let entries = vec!["denylist/global".to_string(), "manual/guardian".to_string()];

        let original_digest = compute_blinded_digest(&digest_key, &token_id, &evidence, &entries)
            .expect("original digest");
        let cloned_digest =
            compute_blinded_digest(&cloned, &token_id, &evidence, &entries).expect("cloned digest");

        assert_eq!(original_digest, cloned_digest);
    }

    #[test]
    fn mint_reports_rng_failure() {
        let mut rng = FailingTryRng;
        let digest_key = ProofTokenDigestKey::new([3; 32]);
        let signing = test_signing_key();
        let evidence = [9u8; 32];
        let params = ProofTokenParams {
            moderation: ModerationAction::Block,
            entry_ids: &["denylist/global"],
            evidence_digest: &evidence,
            issued_at: UNIX_EPOCH + Duration::from_secs(1_714_000_000),
            expires_at: None,
        };

        let err = ProofToken::mint(&mut rng, &digest_key, &signing, &params)
            .expect_err("mint should surface RNG failure");
        match err {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting proof token id");
                assert!(message.contains("failing proof token RNG"));
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn fill_random_rejects_all_zero_token_id_material() {
        let mut rng = FixedTryRng { byte: 0 };
        let mut token_id = [0u8; 16];

        let err = fill_random(&mut rng, "minting proof token id", &mut token_id)
            .expect_err("all-zero proof token id material must fail");

        match err {
            MintError::RandomBytes { operation, message } => {
                assert_eq!(operation, "minting proof token id");
                assert!(message.contains("all-zero material"));
            }
            other => panic!("expected all-zero token id RandomBytes error, got {other:?}"),
        }
    }

    #[test]
    fn decode_truncated_token_prefixes_fail_closed() {
        let mut rng = ChaCha20Rng::seed_from_u64(43);
        let digest_key = ProofTokenDigestKey::new([3; 32]);
        let signing = test_signing_key();
        let evidence = [9u8; 32];
        let params = ProofTokenParams {
            moderation: ModerationAction::Block,
            entry_ids: &["denylist/global", "manual/guardian"],
            evidence_digest: &evidence,
            issued_at: UNIX_EPOCH + Duration::from_secs(1_714_000_000),
            expires_at: Some(UNIX_EPOCH + Duration::from_secs(1_714_000_600)),
        };
        let encoded = ProofToken::mint(&mut rng, &digest_key, &signing, &params)
            .expect("mint")
            .encode();

        for len in 0..encoded.len() {
            assert!(
                ProofToken::decode(&encoded[..len]).is_err(),
                "truncated prefix of length {len} must fail closed"
            );
        }
    }

    #[test]
    fn base64_roundtrip() {
        let mut rng = ChaCha20Rng::seed_from_u64(17);
        let digest_key = ProofTokenDigestKey::new([11; 32]);
        let signing = test_signing_key();
        let evidence = [2u8; 32];
        let params = ProofTokenParams {
            moderation: ModerationAction::Quarantine,
            entry_ids: &["taikai/live/event"],
            evidence_digest: &evidence,
            issued_at: UNIX_EPOCH + Duration::from_secs(1_714_200_000),
            expires_at: Some(UNIX_EPOCH + Duration::from_secs(1_714_200_120)),
        };
        let token = ProofToken::mint(&mut rng, &digest_key, &signing, &params).unwrap();
        let header = token.encode_base64();
        let decoded = ProofToken::decode_base64(&header).unwrap();
        assert_eq!(token, decoded);
    }

    #[test]
    fn decode_base64_rejects_malformed_text_and_invalid_frames() {
        let err = ProofToken::decode_base64("%%%").expect_err("invalid base64 should be rejected");
        assert!(matches!(err, DecodeError::Base64));

        let truncated = encode_base64_url_no_pad(FRAME_MAGIC);
        let err =
            ProofToken::decode_base64(&truncated).expect_err("truncated frame should be rejected");
        assert!(matches!(err, DecodeError::Truncated));
    }

    #[test]
    fn decode_rejects_empty_entries() {
        let token = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: 10,
            expires_at: None,
            entry_ids: Vec::new(),
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };
        let err = ProofToken::decode(&token.encode()).expect_err("empty entries should fail");
        assert!(matches!(err, DecodeError::MissingEntries));
    }

    #[test]
    fn decode_rejects_expiry_before_issue() {
        let token = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: 20,
            expires_at: Some(19),
            entry_ids: vec!["denylist/entry".to_string()],
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };
        let err = ProofToken::decode(&token.encode()).expect_err("invalid expiry should fail");
        assert!(matches!(
            err,
            DecodeError::InvalidExpiry {
                issued_at: 20,
                expires_at: 19
            }
        ));
    }

    #[test]
    fn decode_rejects_unrepresentable_timestamps() {
        let issued_overflow = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: u64::MAX,
            expires_at: None,
            entry_ids: vec!["denylist/entry".to_string()],
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };
        let err = ProofToken::decode(&issued_overflow.encode())
            .expect_err("unrepresentable issued_at should fail closed");
        match err {
            DecodeError::TimestampOutOfRange { field, value } => {
                assert_eq!(field, "issued_at");
                assert_eq!(value, u64::MAX);
            }
            other => panic!("expected timestamp range error, got {other:?}"),
        }

        let expiry_overflow = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: 10,
            expires_at: Some(u64::MAX),
            entry_ids: vec!["denylist/entry".to_string()],
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };
        let err = ProofToken::decode(&expiry_overflow.encode())
            .expect_err("unrepresentable expires_at should fail closed");
        match err {
            DecodeError::TimestampOutOfRange { field, value } => {
                assert_eq!(field, "expires_at");
                assert_eq!(value, u64::MAX);
            }
            other => panic!("expected timestamp range error, got {other:?}"),
        }
    }

    #[test]
    fn timestamp_accessors_fail_closed_on_unrepresentable_values() {
        let token = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: u64::MAX,
            expires_at: Some(u64::MAX),
            entry_ids: vec!["denylist/entry".to_string()],
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };

        assert!(token.checked_issued_at().is_none());
        assert_eq!(token.issued_at(), UNIX_EPOCH);
        assert!(token.checked_expires_at().is_none());
        assert_eq!(token.expires_at(), Some(UNIX_EPOCH));

        let no_expiry = ProofToken {
            expires_at: None,
            ..token
        };
        assert!(no_expiry.checked_expires_at().is_none());
        assert!(no_expiry.expires_at().is_none());
    }

    #[test]
    fn decode_rejects_unknown_flags() {
        let token = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: 10,
            expires_at: None,
            entry_ids: vec!["denylist/entry".to_string()],
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };
        let mut bytes = token.encode();
        bytes[FRAME_MAGIC.len() + 1] = 0x80;
        let err = ProofToken::decode(&bytes).expect_err("unknown flags should fail");
        assert!(matches!(err, DecodeError::InvalidFlags(0x80)));
    }

    #[test]
    fn decode_rejects_all_zero_signature_material() {
        let token = ProofToken {
            token_id: [0x24; 16],
            moderation: ModerationAction::Block,
            issued_at: 10,
            expires_at: None,
            entry_ids: vec!["denylist/entry".to_string()],
            blinded_digest: [0x42; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };

        let err = ProofToken::decode(&token.encode())
            .expect_err("all-zero proof-token signature must fail decoding");

        assert!(matches!(err, DecodeError::InertSignature));
    }

    #[test]
    fn verify_signature_rejects_all_zero_signature_material() {
        let token = ProofToken {
            token_id: [0x24; 16],
            moderation: ModerationAction::Block,
            issued_at: 10,
            expires_at: None,
            entry_ids: vec!["denylist/entry".to_string()],
            blinded_digest: [0x42; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };

        let err = token
            .verify_signature(&test_signing_key().verifying_key())
            .expect_err("all-zero proof-token signature must fail verification");

        assert!(matches!(err, VerificationError::InertSignature));
    }

    #[test]
    fn tampering_detected() {
        let mut rng = ChaCha20Rng::seed_from_u64(99);
        let digest_key = ProofTokenDigestKey::new([5; 32]);
        let signing = test_signing_key();
        let verifying = signing.verifying_key();
        let evidence = [4u8; 32];
        let params = ProofTokenParams {
            moderation: ModerationAction::RateLimit,
            entry_ids: &["rate-limit/geo"],
            evidence_digest: &evidence,
            issued_at: UNIX_EPOCH + Duration::from_secs(1_714_333_333),
            expires_at: Some(UNIX_EPOCH + Duration::from_secs(1_714_333_933)),
        };
        let token = ProofToken::mint(&mut rng, &digest_key, &signing, &params).unwrap();
        token.verify_signature(&verifying).unwrap();
        token.verify_blinded_digest(&digest_key, &evidence).unwrap();

        let mut bytes = token.encode();
        // Flip one byte inside the first entry id.
        let offset = FRAME_MAGIC.len() + 1 + 1 + 1 + 8 + 8 + 16 + 2 + 2;
        bytes[offset] ^= 0x01;
        let decoded = ProofToken::decode(&bytes).unwrap();
        assert!(decoded.verify_signature(&verifying).is_err());
    }

    #[test]
    fn try_encode_rejects_unencodable_direct_entry_count_without_panic() {
        let token = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: 10,
            expires_at: None,
            entry_ids: vec![String::new(); usize::from(u16::MAX) + 1],
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };

        let err = token
            .try_encode()
            .expect_err("oversized direct entry count should not encode");
        assert!(matches!(
            err,
            EncodeError::EntryCountTooLarge {
                max,
                actual
            } if max == usize::from(u16::MAX) && actual == usize::from(u16::MAX) + 1
        ));
        assert!(matches!(
            ProofToken::decode(&token.encode()),
            Err(DecodeError::Truncated)
        ));
        assert!(
            token
                .verify_signature(&test_signing_key().verifying_key())
                .is_err()
        );
    }

    #[test]
    fn unencodable_direct_entry_lengths_fail_closed_without_panic() {
        let digest_key = ProofTokenDigestKey::new([5; 32]);
        let evidence = [4u8; 32];
        let token = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: 10,
            expires_at: None,
            entry_ids: vec!["x".repeat(usize::from(u16::MAX) + 1)],
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };

        let err = token
            .try_encode()
            .expect_err("oversized direct entry should not encode");
        assert!(matches!(
            err,
            EncodeError::EntryTooLong {
                max,
                actual
            } if max == usize::from(u16::MAX) && actual == usize::from(u16::MAX) + 1
        ));
        assert!(matches!(
            ProofToken::decode(&token.encode()),
            Err(DecodeError::Truncated)
        ));
        assert!(matches!(
            token.verify_blinded_digest(&digest_key, &evidence),
            Err(VerificationError::BlindedDigestMismatch)
        ));
    }

    #[test]
    fn verify_signature_rejects_low_order_public_key_signatures() {
        const ED25519_SMALL_ORDER_POINT: [u8; 32] = [
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];

        fn hash_mod_order(
            r: &EdwardsPoint,
            pk_bytes: &[u8; 32],
            msg: &[u8],
            order: usize,
        ) -> usize {
            let mut h = Sha512::new();
            h.update(r.compress().as_bytes());
            h.update(pk_bytes);
            h.update(msg);
            let k = curve25519_dalek::scalar::Scalar::from_hash(h);
            (k.to_bytes()[0] as usize) % order
        }

        let pk = VerifyingKey::from_bytes(&ED25519_SMALL_ORDER_POINT)
            .expect("low-order public key should parse");
        let a_point = pk.to_edwards();
        let mut order = 1usize;
        let mut acc = a_point;
        while !acc.is_identity() {
            acc += a_point;
            order += 1;
            assert!(order <= 8, "torsion order exceeded expected bound");
        }

        let mut torsion_points = Vec::with_capacity(order);
        let mut acc = EdwardsPoint::identity();
        for _ in 0..order {
            torsion_points.push(acc);
            acc += a_point;
        }

        let mut token = ProofToken {
            token_id: [0u8; 16],
            moderation: ModerationAction::Block,
            issued_at: 0,
            expires_at: None,
            entry_ids: vec!["denylist/entry".to_string()],
            blinded_digest: [0u8; 32],
            signature: Signature::from_bytes(&[0u8; SIGNATURE_LENGTH]),
        };

        for counter in 0u32..2048 {
            token.token_id[..4].copy_from_slice(&counter.to_le_bytes());
            let body = token.body_without_signature().expect("body");
            let message = signing_message(&body);

            for (m, r_point) in torsion_points.iter().enumerate() {
                let k_mod = hash_mod_order(r_point, pk.as_bytes(), &message, order);
                let expected_m = (order - k_mod) % order;
                if m == expected_m {
                    let mut sig = [0u8; SIGNATURE_LENGTH];
                    sig[..32].copy_from_slice(r_point.compress().as_bytes());
                    token.signature = Signature::from_bytes(&sig);
                    pk.verify(&message, &token.signature)
                        .expect("non-strict verify accepts low-order signature");
                    assert!(
                        token.verify_signature(&pk).is_err(),
                        "strict verification must reject low-order signature"
                    );
                    return;
                }
            }
        }

        panic!("failed to forge low-order proof token signature");
    }
}
