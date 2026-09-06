//! Exact-eight-slot ML-KEM/XChaCha confidential memo encryption.
//!
//! A memo has one independently random body key and exactly eight shuffled
//! recipient slots. Real and padding slots use the same ML-KEM suite and carry
//! complete encapsulations plus authenticated body-key wraps, so the wire does
//! not advertise a recipient count. This module owns cryptography only; the
//! data-model crate owns the canonical Norito wire types.

use aead::{Aead as _, KeyInit as _, Payload};
use chacha20poly1305::XChaCha20Poly1305;
use hkdf::Hkdf;
use rand::RngCore as _;
use sha3::{Digest as _, Sha3_256};
use soranet_pq::{
    HedgedChaCha20Rng, MlKemError, MlKemKeyPair, MlKemSuite, RngError, decapsulate_mlkem,
    encapsulate_mlkem, generate_mlkem_keypair, generate_mlkem_keypair_from_os,
    hedged_chacha20_rng_from_os,
};
use thiserror::Error;
use zeroize::Zeroizing;

/// Exact number of real-or-padding recipient slots.
pub const CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1: usize = 8;
/// Exact XChaCha20-Poly1305 nonce length.
pub const CONFIDENTIAL_MEMO_NONCE_BYTES_V1: usize = 24;
/// Exact encrypted 32-byte body-key plus Poly1305-tag length.
pub const CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1: usize = 48;
/// Minimum body ciphertext length: an empty plaintext plus its Poly1305 tag.
pub const CONFIDENTIAL_MEMO_TAG_BYTES_V1: usize = 16;
/// Consensus-facing ciphertext allocation cap.
pub const CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1: usize = 64 * 1024;

const MEMO_RNG_PERSONALIZATION_V1: &[u8] = b"iroha.confidential.memo.rng.v1\0";
const WRAP_KDF_SALT_V1: &[u8] = b"iroha.confidential.memo.wrap.hkdf-sha3-256.v1\0";
const WRAP_KDF_INFO_V1: &[u8] = b"iroha.confidential.memo.wrap-key.v1\0";
const WRAP_AAD_DOMAIN_V1: &[u8] = b"iroha.confidential.memo.wrap-aad.v1\0";
const BODY_AAD_DOMAIN_V1: &[u8] = b"iroha.confidential.memo.body-aad.v1\0";

/// Closed first-release ML-KEM suite accepted by confidential memos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfidentialMemoKemSuiteV1 {
    /// FIPS 203 ML-KEM-768.
    MlKem768,
    /// FIPS 203 ML-KEM-1024.
    MlKem1024,
}

impl ConfidentialMemoKemSuiteV1 {
    /// Canonical one-byte wire tag shared with the data model.
    #[must_use]
    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::MlKem768 => 0,
            Self::MlKem1024 => 1,
        }
    }

    /// Underlying reviewed ML-KEM implementation suite.
    #[must_use]
    pub const fn mlkem_suite(self) -> MlKemSuite {
        match self {
            Self::MlKem768 => MlKemSuite::MlKem768,
            Self::MlKem1024 => MlKemSuite::MlKem1024,
        }
    }
}

/// One cryptographically complete recipient or padding slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfidentialMemoCiphertextSlotV1 {
    /// ML-KEM suite used by this slot.
    pub suite: ConfidentialMemoKemSuiteV1,
    /// Suite-sized ML-KEM ciphertext.
    pub encapsulation: Vec<u8>,
    /// XChaCha nonce used to wrap the body key.
    pub wrap_nonce: [u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
    /// Authenticated wrap of the 32-byte body key.
    pub wrapped_body_key: [u8; CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1],
}

/// Cryptographic contents of one exact-eight-slot memo envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfidentialMemoCiphertextV1 {
    /// Eight shuffled, indistinguishable real-or-padding slots.
    pub slots: [ConfidentialMemoCiphertextSlotV1; CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1],
    /// XChaCha nonce used by the encrypted body.
    pub payload_nonce: [u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
    /// Authenticated encrypted body.
    pub ciphertext: Vec<u8>,
}

/// Confidential-memo encryption or opening failure.
#[derive(Debug, Error)]
pub enum ConfidentialMemoErrorV1 {
    /// A memo must have between one and eight real recipients.
    #[error("confidential memo requires between one and eight recipients")]
    RecipientCardinality,
    /// One supplied ML-KEM key or encapsulation is invalid.
    #[error("invalid confidential memo ML-KEM material: {0}")]
    MlKem(#[from] MlKemError),
    /// Required production randomness was unavailable.
    #[error("confidential memo randomness is unavailable: {0}")]
    Randomness(#[from] RngError),
    /// The plaintext would violate the ciphertext cap after authentication.
    #[error("confidential memo plaintext exceeds the V1 ciphertext cap")]
    PlaintextTooLarge,
    /// The supplied envelope violates a fixed V1 shape invariant.
    #[error("malformed confidential memo V1 envelope")]
    MalformedEnvelope,
    /// HKDF could not derive the fixed-size wrapping key.
    #[error("confidential memo wrapping-key derivation failed")]
    Kdf,
    /// XChaCha20-Poly1305 encryption failed.
    #[error("confidential memo encryption failed")]
    Encrypt,
    /// No slot could be authenticated by the supplied recipient secret key.
    #[error("confidential memo has no uniquely authenticated slot for this recipient")]
    RecipientNotFound,
}

/// Generate a production ML-KEM keypair for confidential memos.
///
/// # Errors
///
/// Returns an error if OS entropy is unavailable or the ML-KEM backend fails.
pub fn generate_confidential_memo_keypair_v1(
    suite: ConfidentialMemoKemSuiteV1,
) -> Result<MlKemKeyPair, ConfidentialMemoErrorV1> {
    generate_mlkem_keypair_from_os(suite.mlkem_suite()).map_err(Into::into)
}

/// Encrypt a memo for one to eight recipients using hedged OS entropy.
///
/// All recipients must use the selected suite. Missing slots are populated by
/// fresh dummy keypairs and all eight entries are shuffled before encryption.
///
/// # Errors
///
/// Rejects bad cardinality, malformed public keys, oversized plaintext, RNG
/// failure, or an underlying ML-KEM/XChaCha failure.
pub fn seal_confidential_memo_v1(
    suite: ConfidentialMemoKemSuiteV1,
    recipient_public_keys: &[Vec<u8>],
    plaintext: &[u8],
) -> Result<ConfidentialMemoCiphertextV1, ConfidentialMemoErrorV1> {
    let mut rng = hedged_chacha20_rng_from_os(MEMO_RNG_PERSONALIZATION_V1)?;
    seal_confidential_memo_with_rng_v1(suite, recipient_public_keys, plaintext, &mut rng)
}

/// Encrypt a memo with an explicit hedged RNG.
///
/// This entry point exists for deterministic release KAT generation and for
/// embedders that already own reviewed entropy. Production convenience callers
/// should use [`seal_confidential_memo_v1`].
///
/// # Errors
///
/// Returns the same failures as [`seal_confidential_memo_v1`].
pub fn seal_confidential_memo_with_rng_v1(
    suite: ConfidentialMemoKemSuiteV1,
    recipient_public_keys: &[Vec<u8>],
    plaintext: &[u8],
    rng: &mut HedgedChaCha20Rng,
) -> Result<ConfidentialMemoCiphertextV1, ConfidentialMemoErrorV1> {
    if recipient_public_keys.is_empty()
        || recipient_public_keys.len() > CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1
    {
        return Err(ConfidentialMemoErrorV1::RecipientCardinality);
    }
    if plaintext.len() > CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1 - CONFIDENTIAL_MEMO_TAG_BYTES_V1
    {
        return Err(ConfidentialMemoErrorV1::PlaintextTooLarge);
    }
    let mlkem_suite = suite.mlkem_suite();
    for public_key in recipient_public_keys {
        mlkem_suite.validate_public_key(public_key)?;
    }

    let mut slot_public_keys = recipient_public_keys.to_vec();
    while slot_public_keys.len() < CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1 {
        let dummy = generate_mlkem_keypair(mlkem_suite, rng)?;
        slot_public_keys.push(dummy.public_key().to_vec());
    }
    shuffle(&mut slot_public_keys, rng);

    let mut body_key = Zeroizing::new([0_u8; 32]);
    fill_nonzero(&mut body_key, rng)?;
    let mut payload_nonce = [0_u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1];
    fill_nonzero(&mut payload_nonce, rng)?;

    let mut slots = Vec::with_capacity(CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1);
    for (index, public_key) in slot_public_keys.iter().enumerate() {
        let (shared_secret, encapsulation) = encapsulate_mlkem(mlkem_suite, public_key, rng)?;
        let encapsulation = encapsulation.as_bytes().to_vec();
        let mut wrap_nonce = [0_u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1];
        let mut nonce_is_unique = false;
        for _ in 0..8 {
            fill_nonzero(&mut wrap_nonce, rng)?;
            if slots
                .iter()
                .all(|slot: &ConfidentialMemoCiphertextSlotV1| slot.wrap_nonce != wrap_nonce)
            {
                nonce_is_unique = true;
                break;
            }
        }
        if !nonce_is_unique {
            return Err(ConfidentialMemoErrorV1::Randomness(RngError));
        }
        let wrap_key = derive_wrap_key(
            shared_secret.as_bytes(),
            suite,
            index,
            &encapsulation,
            &wrap_nonce,
            &payload_nonce,
        )?;
        let wrap_aad = wrap_aad(suite, index, &encapsulation, &wrap_nonce, &payload_nonce);
        let wrapped = xchacha_encrypt(
            wrap_key.as_slice(),
            &wrap_nonce,
            &wrap_aad,
            body_key.as_ref(),
        )?;
        let wrapped_body_key = wrapped
            .try_into()
            .map_err(|_| ConfidentialMemoErrorV1::Encrypt)?;
        slots.push(ConfidentialMemoCiphertextSlotV1 {
            suite,
            encapsulation,
            wrap_nonce,
            wrapped_body_key,
        });
    }
    let slots: [ConfidentialMemoCiphertextSlotV1; CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1] = slots
        .try_into()
        .map_err(|_| ConfidentialMemoErrorV1::MalformedEnvelope)?;
    let aad = body_aad(&slots, &payload_nonce);
    let ciphertext = xchacha_encrypt(body_key.as_slice(), &payload_nonce, &aad, plaintext)?;
    Ok(ConfidentialMemoCiphertextV1 {
        slots,
        payload_nonce,
        ciphertext,
    })
}

/// Open one exact-eight-slot memo with an ML-KEM recipient secret key.
///
/// Every matching-suite slot is attempted, and success requires exactly one
/// authenticated body-key wrap and body. This avoids a sender-controlled first
/// match and fails closed on duplicated recipient slots.
///
/// # Errors
///
/// Rejects malformed envelopes or secret keys and returns
/// [`ConfidentialMemoErrorV1::RecipientNotFound`] unless exactly one slot and
/// body authenticate.
pub fn open_confidential_memo_v1(
    envelope: &ConfidentialMemoCiphertextV1,
    suite: ConfidentialMemoKemSuiteV1,
    recipient_secret_key: &[u8],
) -> Result<Vec<u8>, ConfidentialMemoErrorV1> {
    validate_envelope(envelope)?;
    suite
        .mlkem_suite()
        .validate_secret_key(recipient_secret_key)?;
    let body_aad = body_aad(&envelope.slots, &envelope.payload_nonce);
    let mut opened: Option<Vec<u8>> = None;
    for (index, slot) in envelope.slots.iter().enumerate() {
        if slot.suite != suite {
            continue;
        }
        let shared = decapsulate_mlkem(
            suite.mlkem_suite(),
            recipient_secret_key,
            &slot.encapsulation,
        )?;
        let wrap_key = derive_wrap_key(
            shared.as_bytes(),
            suite,
            index,
            &slot.encapsulation,
            &slot.wrap_nonce,
            &envelope.payload_nonce,
        )?;
        let aad = wrap_aad(
            suite,
            index,
            &slot.encapsulation,
            &slot.wrap_nonce,
            &envelope.payload_nonce,
        );
        let Some(body_key) = xchacha_decrypt(
            wrap_key.as_slice(),
            &slot.wrap_nonce,
            &aad,
            &slot.wrapped_body_key,
        ) else {
            continue;
        };
        let body_key = Zeroizing::new(body_key);
        if body_key.len() != 32 || body_key.iter().all(|byte| *byte == 0) {
            continue;
        }
        let Some(plaintext) = xchacha_decrypt(
            body_key.as_slice(),
            &envelope.payload_nonce,
            &body_aad,
            &envelope.ciphertext,
        ) else {
            continue;
        };
        if opened.replace(plaintext).is_some() {
            return Err(ConfidentialMemoErrorV1::RecipientNotFound);
        }
    }
    opened.ok_or(ConfidentialMemoErrorV1::RecipientNotFound)
}

fn validate_envelope(
    envelope: &ConfidentialMemoCiphertextV1,
) -> Result<(), ConfidentialMemoErrorV1> {
    if envelope.payload_nonce.iter().all(|byte| *byte == 0)
        || envelope.ciphertext.len() < CONFIDENTIAL_MEMO_TAG_BYTES_V1
        || envelope.ciphertext.len() > CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1
    {
        return Err(ConfidentialMemoErrorV1::MalformedEnvelope);
    }
    for (index, slot) in envelope.slots.iter().enumerate() {
        slot.suite
            .mlkem_suite()
            .validate_ciphertext(&slot.encapsulation)?;
        if slot.wrap_nonce.iter().all(|byte| *byte == 0)
            || slot.wrapped_body_key.iter().all(|byte| *byte == 0)
            || envelope.slots[..index].contains(slot)
        {
            return Err(ConfidentialMemoErrorV1::MalformedEnvelope);
        }
    }
    Ok(())
}

fn derive_wrap_key(
    shared_secret: &[u8],
    suite: ConfidentialMemoKemSuiteV1,
    index: usize,
    encapsulation: &[u8],
    wrap_nonce: &[u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
    payload_nonce: &[u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
) -> Result<Zeroizing<[u8; 32]>, ConfidentialMemoErrorV1> {
    let hkdf = Hkdf::<Sha3_256>::new(Some(WRAP_KDF_SALT_V1), shared_secret);
    let mut info = Vec::with_capacity(WRAP_KDF_INFO_V1.len() + 1 + 1 + 32 + 24 + 24);
    info.extend_from_slice(WRAP_KDF_INFO_V1);
    info.push(suite.wire_tag());
    info.push(u8::try_from(index).expect("eight recipient slots fit u8"));
    info.extend_from_slice(&Sha3_256::digest(encapsulation));
    info.extend_from_slice(wrap_nonce);
    info.extend_from_slice(payload_nonce);
    let mut key = Zeroizing::new([0_u8; 32]);
    hkdf.expand(&info, key.as_mut())
        .map_err(|_| ConfidentialMemoErrorV1::Kdf)?;
    if key.iter().all(|byte| *byte == 0) {
        return Err(ConfidentialMemoErrorV1::Kdf);
    }
    Ok(key)
}

fn wrap_aad(
    suite: ConfidentialMemoKemSuiteV1,
    index: usize,
    encapsulation: &[u8],
    wrap_nonce: &[u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
    payload_nonce: &[u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(WRAP_AAD_DOMAIN_V1.len() + 1 + 1 + 32 + 24 + 24);
    aad.extend_from_slice(WRAP_AAD_DOMAIN_V1);
    aad.push(suite.wire_tag());
    aad.push(u8::try_from(index).expect("eight recipient slots fit u8"));
    aad.extend_from_slice(&Sha3_256::digest(encapsulation));
    aad.extend_from_slice(wrap_nonce);
    aad.extend_from_slice(payload_nonce);
    aad
}

fn body_aad(
    slots: &[ConfidentialMemoCiphertextSlotV1; CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1],
    payload_nonce: &[u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(BODY_AAD_DOMAIN_V1.len() + 24 + 8 * (1 + 32 + 24 + 48));
    aad.extend_from_slice(BODY_AAD_DOMAIN_V1);
    for slot in slots {
        aad.push(slot.suite.wire_tag());
        aad.extend_from_slice(&Sha3_256::digest(&slot.encapsulation));
        aad.extend_from_slice(&slot.wrap_nonce);
        aad.extend_from_slice(&slot.wrapped_body_key);
    }
    aad.extend_from_slice(payload_nonce);
    aad
}

fn xchacha_encrypt(
    key: &[u8],
    nonce: &[u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, ConfidentialMemoErrorV1> {
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).map_err(|_| ConfidentialMemoErrorV1::Encrypt)?;
    let nonce = aead::Nonce::<XChaCha20Poly1305>::try_from(nonce.as_slice())
        .map_err(|_| ConfidentialMemoErrorV1::Encrypt)?;
    cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| ConfidentialMemoErrorV1::Encrypt)
}

fn xchacha_decrypt(
    key: &[u8],
    nonce: &[u8; CONFIDENTIAL_MEMO_NONCE_BYTES_V1],
    aad: &[u8],
    ciphertext: &[u8],
) -> Option<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).ok()?;
    let nonce = aead::Nonce::<XChaCha20Poly1305>::try_from(nonce.as_slice()).ok()?;
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .ok()
}

fn fill_nonzero<const N: usize>(
    value: &mut [u8; N],
    rng: &mut HedgedChaCha20Rng,
) -> Result<(), ConfidentialMemoErrorV1> {
    for _ in 0..8 {
        rng.fill_bytes(value);
        if value.iter().any(|byte| *byte != 0) {
            return Ok(());
        }
    }
    Err(ConfidentialMemoErrorV1::Randomness(RngError))
}

fn shuffle<T>(values: &mut [T], rng: &mut HedgedChaCha20Rng) {
    for upper in (1..values.len()).rev() {
        let index = unbiased_index(upper + 1, rng);
        values.swap(upper, index);
    }
}

fn unbiased_index(bound: usize, rng: &mut HedgedChaCha20Rng) -> usize {
    let bound = u64::try_from(bound).expect("memo slot bound fits u64");
    let rejection_floor = u64::MAX - u64::MAX % bound;
    loop {
        let candidate = rng.next_u64();
        if candidate < rejection_floor {
            return usize::try_from(candidate % bound).expect("bounded memo index fits usize");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soranet_pq::{HedgedRngSeed, deterministic_chacha20_rng};

    fn rng(label: u8) -> HedgedChaCha20Rng {
        deterministic_chacha20_rng(
            HedgedRngSeed::from_entropy([label; 32]),
            b"iroha.confidential.memo.kat.v1",
        )
    }

    #[test]
    fn every_recipient_opens_one_exact_eight_slot_memo() {
        let suite = ConfidentialMemoKemSuiteV1::MlKem768;
        let mut key_rng = rng(1);
        let first = generate_mlkem_keypair(suite.mlkem_suite(), &mut key_rng).expect("first key");
        let second = generate_mlkem_keypair(suite.mlkem_suite(), &mut key_rng).expect("second key");
        let public_keys = vec![first.public_key().to_vec(), second.public_key().to_vec()];
        let mut seal_rng = rng(2);
        let envelope = seal_confidential_memo_with_rng_v1(
            suite,
            &public_keys,
            b"exact-eight-slot memo",
            &mut seal_rng,
        )
        .expect("seal memo");
        assert_eq!(envelope.slots.len(), CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1);
        assert!(envelope.slots.iter().all(|slot| slot.suite == suite));
        assert_eq!(
            open_confidential_memo_v1(&envelope, suite, first.secret_key()).expect("first opens"),
            b"exact-eight-slot memo"
        );
        assert_eq!(
            open_confidential_memo_v1(&envelope, suite, second.secret_key()).expect("second opens"),
            b"exact-eight-slot memo"
        );
    }

    #[test]
    fn substitution_and_wrong_recipient_fail_closed() {
        let suite = ConfidentialMemoKemSuiteV1::MlKem1024;
        let mut key_rng = rng(3);
        let recipient =
            generate_mlkem_keypair(suite.mlkem_suite(), &mut key_rng).expect("recipient key");
        let outsider = generate_mlkem_keypair(suite.mlkem_suite(), &mut key_rng).expect("outsider");
        let mut seal_rng = rng(4);
        let mut envelope = seal_confidential_memo_with_rng_v1(
            suite,
            &[recipient.public_key().to_vec()],
            b"bound memo",
            &mut seal_rng,
        )
        .expect("seal memo");
        assert!(
            open_confidential_memo_v1(&envelope, suite, outsider.secret_key()).is_err(),
            "an unrelated recipient must not authenticate any padding slot"
        );
        envelope.slots.swap(0, 1);
        assert!(
            open_confidential_memo_v1(&envelope, suite, recipient.secret_key()).is_err(),
            "slot positions are authenticated by both wrap and body AAD"
        );
    }

    #[test]
    fn deterministic_kat_seed_is_byte_identical() {
        let suite = ConfidentialMemoKemSuiteV1::MlKem768;
        let mut key_rng = rng(7);
        let recipient =
            generate_mlkem_keypair(suite.mlkem_suite(), &mut key_rng).expect("recipient key");
        let recipients = [recipient.public_key().to_vec()];
        let mut first_rng = rng(8);
        let mut second_rng = rng(8);
        let first = seal_confidential_memo_with_rng_v1(
            suite,
            &recipients,
            b"deterministic release KAT",
            &mut first_rng,
        )
        .expect("first seal");
        let second = seal_confidential_memo_with_rng_v1(
            suite,
            &recipients,
            b"deterministic release KAT",
            &mut second_rng,
        )
        .expect("second seal");
        assert_eq!(first, second);
    }

    #[test]
    fn cardinality_and_ciphertext_caps_are_enforced_before_work() {
        let suite = ConfidentialMemoKemSuiteV1::MlKem768;
        let mut seal_rng = rng(5);
        assert!(matches!(
            seal_confidential_memo_with_rng_v1(suite, &[], b"memo", &mut seal_rng),
            Err(ConfidentialMemoErrorV1::RecipientCardinality)
        ));
        let mut key_rng = rng(6);
        let recipient =
            generate_mlkem_keypair(suite.mlkem_suite(), &mut key_rng).expect("recipient key");
        assert!(matches!(
            seal_confidential_memo_with_rng_v1(
                suite,
                &[recipient.public_key().to_vec()],
                &vec![
                    0;
                    CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1 - CONFIDENTIAL_MEMO_TAG_BYTES_V1 + 1
                ],
                &mut seal_rng,
            ),
            Err(ConfidentialMemoErrorV1::PlaintextTooLarge)
        ));
    }
}
