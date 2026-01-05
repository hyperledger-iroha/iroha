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

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use blake3::Hasher;
use ed25519_dalek::{SIGNATURE_LENGTH, Signature, Signer, SigningKey, VerifyingKey};
use rand::{CryptoRng, RngCore};
use thiserror::Error;

const FRAME_MAGIC: &[u8; 4] = b"SFGT";
const DIGEST_DOMAIN: &[u8] = b"sorafs.proof_token.digest.v1";
const SIGNING_DOMAIN: &[u8] = b"sorafs.proof_token.sign.v1";
const MAX_ENTRY_IDS: usize = 32;
const MAX_ENTRY_LEN: usize = 255;
const FLAG_HAS_EXPIRY: u8 = 0x01;

/// Secret used to derive the blinded digest portion of a token body.
#[derive(Clone, Copy)]
pub struct ProofTokenDigestKey([u8; 32]);

impl ProofTokenDigestKey {
    /// Construct a new digest key from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    const fn as_bytes(&self) -> &[u8; 32] {
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
        if self.cursor + len > self.bytes.len() {
            return Err(DecodeError::Truncated);
        }
        let slice = &self.bytes[self.cursor..self.cursor + len];
        self.cursor += len;
        Ok(slice)
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
    pub fn mint<R: RngCore + CryptoRng>(
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
        rng.fill_bytes(&mut token_id);

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
            compute_blinded_digest(digest_key, &token_id, params.evidence_digest, &entry_ids);

        let mut token = Self {
            token_id,
            moderation: params.moderation,
            issued_at,
            expires_at,
            entry_ids,
            blinded_digest,
            signature: Signature::from_bytes(&[0; SIGNATURE_LENGTH]),
        };
        let message = signing_message(&token.body_without_signature());
        token.signature = signing_key.sign(&message);
        Ok(token)
    }

    /// Serialize the token frame.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut body = self.body_without_signature();
        let sig_bytes = self.signature.to_bytes();
        let sig_len = u16::try_from(sig_bytes.len()).expect("ed25519 signature fits in u16");
        body.extend_from_slice(&sig_len.to_be_bytes());
        body.extend_from_slice(&sig_bytes);

        let mut out = Vec::with_capacity(FRAME_MAGIC.len() + body.len());
        out.extend_from_slice(FRAME_MAGIC);
        out.extend_from_slice(&body);
        out
    }

    /// Serialize the token as URL-safe base64 (header-friendly).
    #[must_use]
    pub fn encode_base64(&self) -> String {
        let payload = self.encode();
        let mut buffer = vec![0u8; base64_encoded_len(payload.len())];
        let written = URL_SAFE_NO_PAD
            .encode_slice(payload, &mut buffer)
            .expect("buffer sized using deterministic formula");
        buffer.truncate(written);
        String::from_utf8(buffer).expect("base64 output must be ASCII")
    }

    /// Decode a token from its binary frame.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] when the payload is truncated, malformed, or
    /// uses an unsupported version. Also enforces mint invariants such as
    /// non-empty entry lists and `expires_at` strictly after `issued_at`.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < FRAME_MAGIC.len() + 2 {
            return Err(DecodeError::Truncated);
        }
        if &bytes[..FRAME_MAGIC.len()] != FRAME_MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let mut reader = FrameReader::new(bytes, FRAME_MAGIC.len());

        let version = reader.take(1)?[0];
        if version != Self::VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }

        let flags = reader.take(1)?[0];
        let moderation = ModerationAction::from_u8(reader.take(1)?[0]);
        let issued_at = u64::from_be_bytes(reader.take(8)?.try_into().unwrap());
        let expires_at = if flags & FLAG_HAS_EXPIRY == FLAG_HAS_EXPIRY {
            Some(u64::from_be_bytes(reader.take(8)?.try_into().unwrap()))
        } else {
            None
        };
        if let Some(expires_at) = expires_at {
            if expires_at <= issued_at {
                return Err(DecodeError::InvalidExpiry {
                    issued_at,
                    expires_at,
                });
            }
        }

        let mut token_id = [0u8; 16];
        token_id.copy_from_slice(reader.take(16)?);

        let entry_count = u16::from_be_bytes(reader.take(2)?.try_into().unwrap()) as usize;
        if entry_count == 0 {
            return Err(DecodeError::MissingEntries);
        }
        if entry_count > MAX_ENTRY_IDS {
            return Err(DecodeError::TooManyEntries(entry_count));
        }
        let mut entry_ids = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let len = u16::from_be_bytes(reader.take(2)?.try_into().unwrap()) as usize;
            if len == 0 || len > MAX_ENTRY_LEN {
                return Err(DecodeError::InvalidEntryLength(len));
            }
            let entry = reader.take(len)?;
            let entry = std::str::from_utf8(entry).map_err(|_| DecodeError::InvalidUtf8)?;
            entry_ids.push(entry.to_owned());
        }

        let mut blinded_digest = [0u8; 32];
        blinded_digest.copy_from_slice(reader.take(32)?);

        let sig_len = u16::from_be_bytes(reader.take(2)?.try_into().unwrap()) as usize;
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
        if s.len() % 4 == 1 {
            return Err(DecodeError::Base64);
        }
        let mut buffer = vec![0u8; base64_decoded_capacity(s.len())];
        let written = URL_SAFE_NO_PAD
            .decode_slice(s.as_bytes(), &mut buffer)
            .map_err(|_| DecodeError::Base64)?;
        buffer.truncate(written);
        Self::decode(&buffer)
    }

    /// Return the moderation action classification.
    #[must_use]
    pub fn moderation(&self) -> ModerationAction {
        self.moderation
    }

    /// UNIX timestamp (seconds) describing when the token was issued.
    #[must_use]
    pub fn issued_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.issued_at)
    }

    /// Optional expiry timestamp.
    #[must_use]
    pub fn expires_at(&self) -> Option<SystemTime> {
        self.expires_at
            .map(|ts| UNIX_EPOCH + Duration::from_secs(ts))
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
        let message = signing_message(&self.body_without_signature());
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
            compute_blinded_digest(digest_key, &self.token_id, evidence_digest, &self.entry_ids);
        if expected == self.blinded_digest {
            Ok(())
        } else {
            Err(VerificationError::BlindedDigestMismatch)
        }
    }

    fn body_without_signature(&self) -> Vec<u8> {
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
        let entry_count = u16::try_from(self.entry_ids.len()).expect("entry count fits in u16");
        out.extend_from_slice(&entry_count.to_be_bytes());
        for entry in &self.entry_ids {
            let entry_bytes = entry.as_bytes();
            let len = u16::try_from(entry_bytes.len()).expect("entry id fits in u16");
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(entry_bytes);
        }
        out.extend_from_slice(&self.blinded_digest);
        out
    }
}

/// Errors surfaced when minting new tokens.
#[derive(Debug, Clone, Copy, Error)]
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
    /// Signature length prefix did not match the trailing bytes.
    #[error("signature length mismatch (expected {expected}, actual {actual})")]
    InvalidSignatureLength {
        /// Signature size encoded inside the frame.
        expected: usize,
        /// Actual bytes remaining in the frame.
        actual: usize,
    },
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
    /// Secret re-computed digest did not match the token body.
    #[error("blinded digest mismatch")]
    BlindedDigestMismatch,
}

fn to_unix_seconds(time: SystemTime) -> Result<u64, MintError> {
    time.duration_since(UNIX_EPOCH)
        .map_err(|_| MintError::TimestampOutOfRange)
        .map(|duration| duration.as_secs())
}

fn compute_blinded_digest(
    digest_key: &ProofTokenDigestKey,
    token_id: &[u8; 16],
    evidence_digest: &[u8; 32],
    entries: &[String],
) -> [u8; 32] {
    let mut hasher = Hasher::new_keyed(digest_key.as_bytes());
    hasher.update(DIGEST_DOMAIN);
    hasher.update(token_id);
    hasher.update(evidence_digest);
    for entry in entries {
        let entry_bytes = entry.as_bytes();
        let len = u16::try_from(entry_bytes.len()).expect("entry id fits into u16");
        hasher.update(&len.to_be_bytes());
        hasher.update(entry_bytes);
    }
    hasher.finalize().into()
}

fn signing_message(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(SIGNING_DOMAIN.len() + body.len());
    out.extend_from_slice(SIGNING_DOMAIN);
    out.extend_from_slice(body);
    out
}

fn base64_encoded_len(input_len: usize) -> usize {
    let blocks = input_len / 3;
    let rem = input_len % 3;
    blocks * 4
        + match rem {
            0 => 0,
            1 => 2,
            _ => 3,
        }
}

fn base64_decoded_capacity(encoded_len: usize) -> usize {
    let blocks = encoded_len / 4;
    let rem = encoded_len % 4;
    blocks * 3
        + match rem {
            2 => 1,
            3 => 2,
            _ => 0,
        }
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
    use sha2::{Digest, Sha512};

    use super::*;

    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

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
            acc = acc + a_point;
            order += 1;
            assert!(order <= 8, "torsion order exceeded expected bound");
        }

        let mut torsion_points = Vec::with_capacity(order);
        let mut acc = EdwardsPoint::identity();
        for _ in 0..order {
            torsion_points.push(acc);
            acc = acc + a_point;
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
            let message = signing_message(&token.body_without_signature());

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
