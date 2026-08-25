//! Strict receiver-only encryption for one Offline Cash V1 credit opening.
//!
//! The P-256 device key authorizes request and acknowledgement signatures. It
//! is deliberately not reused for key agreement. A signed request carries one
//! distinct strict X25519 public key, and this module is the sole Core owner of
//! the fixed X25519 + XChaCha20-Poly1305 credit-envelope codec.

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead as _, KeyInit as _, Payload},
};
use iroha_data_model::offline::{
    OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1, OfflineCashPaymentRequestV1,
    OfflineCashTransferStatementV1, offline_cash_receiver_key_reference_v1,
    validate_offline_cash_recipient_encryption_public_key_v1,
};
use rand_core_06::{CryptoRng, RngCore};
use sha2::{Digest as _, Sha256};
use zeroize::{Zeroize as _, Zeroizing};

use crate::privacy_engines::{
    prover_randomness::HealthCheckedCryptoRngV1,
    x25519_wallet::{x25519_public_key_v1, x25519_shared_secret_v1},
};

use super::{
    DecryptedCreditOpeningOwnerV1, Digest, PendingOwnerV1, StateTransitionErrorV1,
    send::SendSplitPlanV1,
};

const ENVELOPE_MAGIC_V1: [u8; 4] = *b"KCE1";
const PLAINTEXT_MAGIC_V1: [u8; 4] = *b"KCO1";
const ENVELOPE_VERSION_V1: u16 = 1;
const ENVELOPE_PREFIX_BYTES_V1: usize = ENVELOPE_MAGIC_V1.len() + core::mem::size_of::<u16>();
const EPHEMERAL_PUBLIC_KEY_BYTES_V1: usize = 32;
const NONCE_BYTES_V1: usize = 24;
const OPENING_BYTES_V1: usize = 32;
const PLAINTEXT_BYTES_V1: usize =
    PLAINTEXT_MAGIC_V1.len() + core::mem::size_of::<u16>() + OPENING_BYTES_V1;
const AUTHENTICATION_TAG_BYTES_V1: usize = 16;
const AUTHENTICATED_BYTES_V1: usize = PLAINTEXT_BYTES_V1 + AUTHENTICATION_TAG_BYTES_V1;
/// Exact bytes in every valid first-release encrypted credit.
pub(crate) const OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES_V1: usize = ENVELOPE_PREFIX_BYTES_V1
    + EPHEMERAL_PUBLIC_KEY_BYTES_V1
    + NONCE_BYTES_V1
    + AUTHENTICATED_BYTES_V1;
const ENTROPY_BYTES_V1: usize = 64;
const CREDIT_AAD_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:credit-aead-aad";
const CREDIT_KEY_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:credit-aead-key";

const _: () = assert!(OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES_V1 == 116);
const _: () =
    assert!(OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES_V1 <= OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1);

#[derive(Clone, Copy)]
struct CreditCipherBindingV1 {
    release_id: Digest,
    request_digest: Digest,
    transition_digest: Digest,
    credit_commitment: Digest,
    recipient_key_reference: Digest,
    recipient_encryption_public_key: Digest,
}

impl CreditCipherBindingV1 {
    fn validate(self) -> Result<Self, StateTransitionErrorV1> {
        if [
            self.release_id,
            self.request_digest,
            self.transition_digest,
            self.credit_commitment,
            self.recipient_key_reference,
            self.recipient_encryption_public_key,
        ]
        .into_iter()
        .any(|value| value == [0; 32])
            || validate_offline_cash_recipient_encryption_public_key_v1(
                self.recipient_encryption_public_key,
            )
            .is_err()
        {
            return Err(StateTransitionErrorV1::EncryptedOpeningMismatch);
        }
        Ok(self)
    }
}

fn append_framed(destination: &mut Vec<u8>, field: &[u8]) {
    destination.extend_from_slice(
        &u64::try_from(field.len())
            .expect("Offline Cash credit AAD field width fits u64")
            .to_le_bytes(),
    );
    destination.extend_from_slice(field);
}

fn credit_aad_v1(
    binding: CreditCipherBindingV1,
    ephemeral_public_key: Digest,
) -> Result<Zeroizing<Vec<u8>>, StateTransitionErrorV1> {
    let binding = binding.validate()?;
    validate_offline_cash_recipient_encryption_public_key_v1(ephemeral_public_key)
        .map_err(|_| StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    let envelope_version = ENVELOPE_VERSION_V1.to_le_bytes();
    let mut aad = Zeroizing::new(Vec::with_capacity(336));
    for field in [
        CREDIT_AAD_DOMAIN_V1,
        ENVELOPE_MAGIC_V1.as_slice(),
        envelope_version.as_slice(),
        binding.release_id.as_slice(),
        binding.request_digest.as_slice(),
        binding.transition_digest.as_slice(),
        binding.credit_commitment.as_slice(),
        binding.recipient_key_reference.as_slice(),
        binding.recipient_encryption_public_key.as_slice(),
        ephemeral_public_key.as_slice(),
    ] {
        append_framed(&mut aad, field);
    }
    Ok(aad)
}

fn credit_key_v1(shared_secret: &[u8; 32], aad: &[u8]) -> Zeroizing<Digest> {
    let mut hasher = Sha256::new();
    append_framed_to_hasher(&mut hasher, CREDIT_KEY_DOMAIN_V1);
    append_framed_to_hasher(&mut hasher, shared_secret);
    append_framed_to_hasher(&mut hasher, aad);
    Zeroizing::new(hasher.finalize().into())
}

fn append_framed_to_hasher(hasher: &mut Sha256, field: &[u8]) {
    hasher.update(
        u64::try_from(field.len())
            .expect("Offline Cash credit KDF field width fits u64")
            .to_le_bytes(),
    );
    hasher.update(field);
}

fn encode_plaintext_v1(opening: &Digest) -> Result<Zeroizing<Vec<u8>>, StateTransitionErrorV1> {
    if *opening == [0; 32] {
        return Err(StateTransitionErrorV1::CreditMismatch);
    }
    let mut plaintext = Zeroizing::new(Vec::with_capacity(PLAINTEXT_BYTES_V1));
    plaintext.extend_from_slice(&PLAINTEXT_MAGIC_V1);
    plaintext.extend_from_slice(&ENVELOPE_VERSION_V1.to_le_bytes());
    plaintext.extend_from_slice(opening);
    if plaintext.len() != PLAINTEXT_BYTES_V1 {
        return Err(StateTransitionErrorV1::CreditMismatch);
    }
    Ok(plaintext)
}

fn decode_plaintext_v1(
    plaintext: &mut Zeroizing<Vec<u8>>,
) -> Result<Zeroizing<Digest>, StateTransitionErrorV1> {
    let envelope_version = ENVELOPE_VERSION_V1.to_le_bytes();
    if plaintext.len() != PLAINTEXT_BYTES_V1
        || plaintext.get(..PLAINTEXT_MAGIC_V1.len()) != Some(PLAINTEXT_MAGIC_V1.as_slice())
        || plaintext.get(PLAINTEXT_MAGIC_V1.len()..ENVELOPE_PREFIX_BYTES_V1)
            != Some(envelope_version.as_slice())
    {
        return Err(StateTransitionErrorV1::EncryptedOpeningMismatch);
    }
    let opening: Digest = plaintext
        .get(ENVELOPE_PREFIX_BYTES_V1..)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    if opening == [0; 32] {
        return Err(StateTransitionErrorV1::EncryptedOpeningMismatch);
    }
    plaintext.zeroize();
    Ok(Zeroizing::new(opening))
}

fn encryption_entropy_is_healthy_v1(entropy: &[u8; ENTROPY_BYTES_V1]) -> bool {
    let (ephemeral_secret, remainder) = entropy.split_at(32);
    let nonce = &remainder[..NONCE_BYTES_V1];
    !ephemeral_secret.iter().all(|byte| *byte == 0)
        && !nonce.iter().all(|byte| *byte == 0)
        && ephemeral_secret[..NONCE_BYTES_V1] != *nonce
}

fn encrypt_credit_opening_v1(
    rng: &mut (impl CryptoRng + RngCore),
    binding: CreditCipherBindingV1,
    opening: &Digest,
) -> Result<Vec<u8>, StateTransitionErrorV1> {
    let binding = binding.validate()?;
    let plaintext = encode_plaintext_v1(opening)?;
    let mut checked_rng = HealthCheckedCryptoRngV1::new(rng)
        .map_err(|_| StateTransitionErrorV1::CreditEncryptionUnavailable)?;
    let mut entropy = Zeroizing::new([0_u8; ENTROPY_BYTES_V1]);
    checked_rng
        .try_fill_bytes(entropy.as_mut())
        .map_err(|_| StateTransitionErrorV1::CreditEncryptionUnavailable)?;
    if !encryption_entropy_is_healthy_v1(&entropy) {
        return Err(StateTransitionErrorV1::CreditEncryptionUnavailable);
    }
    let ephemeral_secret: &Digest = entropy[..32]
        .try_into()
        .map_err(|_| StateTransitionErrorV1::CreditEncryptionUnavailable)?;
    let nonce_bytes: &[u8; NONCE_BYTES_V1] = entropy[32..32 + NONCE_BYTES_V1]
        .try_into()
        .map_err(|_| StateTransitionErrorV1::CreditEncryptionUnavailable)?;
    let ephemeral_public_key = x25519_public_key_v1(ephemeral_secret)
        .map_err(|_| StateTransitionErrorV1::CreditEncryptionUnavailable)?;
    let shared = x25519_shared_secret_v1(ephemeral_secret, binding.recipient_encryption_public_key)
        .map_err(|_| StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    let aad = credit_aad_v1(binding, ephemeral_public_key)?;
    let key = credit_key_v1(&shared, &aad);
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_slice())
        .map_err(|_| StateTransitionErrorV1::CreditEncryptionUnavailable)?;
    let nonce: &XNonce = nonce_bytes
        .as_slice()
        .try_into()
        .map_err(|_| StateTransitionErrorV1::CreditEncryptionUnavailable)?;
    let authenticated = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext.as_slice(),
                aad: aad.as_slice(),
            },
        )
        .map_err(|_| StateTransitionErrorV1::CreditEncryptionUnavailable)?;
    if authenticated.len() != AUTHENTICATED_BYTES_V1 {
        return Err(StateTransitionErrorV1::CreditEncryptionUnavailable);
    }
    let mut envelope = Vec::with_capacity(OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES_V1);
    envelope.extend_from_slice(&ENVELOPE_MAGIC_V1);
    envelope.extend_from_slice(&ENVELOPE_VERSION_V1.to_le_bytes());
    envelope.extend_from_slice(&ephemeral_public_key);
    envelope.extend_from_slice(nonce_bytes);
    envelope.extend_from_slice(&authenticated);
    if envelope.len() != OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES_V1 {
        return Err(StateTransitionErrorV1::CreditEncryptionUnavailable);
    }
    Ok(envelope)
}

fn parse_envelope_v1(
    encrypted_credit: &[u8],
) -> Result<(Digest, &[u8; NONCE_BYTES_V1], &[u8]), StateTransitionErrorV1> {
    let envelope_version = ENVELOPE_VERSION_V1.to_le_bytes();
    if encrypted_credit.len() != OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES_V1
        || encrypted_credit.get(..ENVELOPE_MAGIC_V1.len()) != Some(ENVELOPE_MAGIC_V1.as_slice())
        || encrypted_credit.get(ENVELOPE_MAGIC_V1.len()..ENVELOPE_PREFIX_BYTES_V1)
            != Some(envelope_version.as_slice())
    {
        return Err(StateTransitionErrorV1::EncryptedOpeningMismatch);
    }
    let ephemeral_start = ENVELOPE_PREFIX_BYTES_V1;
    let nonce_start = ephemeral_start + EPHEMERAL_PUBLIC_KEY_BYTES_V1;
    let authenticated_start = nonce_start + NONCE_BYTES_V1;
    let ephemeral_public_key: Digest = encrypted_credit
        .get(ephemeral_start..nonce_start)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    validate_offline_cash_recipient_encryption_public_key_v1(ephemeral_public_key)
        .map_err(|_| StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    let nonce: &[u8; NONCE_BYTES_V1] = encrypted_credit
        .get(nonce_start..authenticated_start)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    let authenticated = encrypted_credit
        .get(authenticated_start..)
        .ok_or(StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    if nonce.iter().all(|byte| *byte == 0)
        || authenticated.len() != AUTHENTICATED_BYTES_V1
        || authenticated.iter().all(|byte| *byte == 0)
    {
        return Err(StateTransitionErrorV1::EncryptedOpeningMismatch);
    }
    Ok((ephemeral_public_key, nonce, authenticated))
}

fn decrypt_credit_opening_v1(
    binding: CreditCipherBindingV1,
    encrypted_credit: &[u8],
    recipient_secret_key: &Digest,
) -> Result<Zeroizing<Digest>, StateTransitionErrorV1> {
    let binding = binding.validate()?;
    if x25519_public_key_v1(recipient_secret_key)
        .map_err(|_| StateTransitionErrorV1::EncryptedOpeningMismatch)?
        != binding.recipient_encryption_public_key
    {
        return Err(StateTransitionErrorV1::EncryptedOpeningMismatch);
    }
    let (ephemeral_public_key, nonce_bytes, authenticated) = parse_envelope_v1(encrypted_credit)?;
    let shared = x25519_shared_secret_v1(recipient_secret_key, ephemeral_public_key)
        .map_err(|_| StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    let aad = credit_aad_v1(binding, ephemeral_public_key)?;
    let key = credit_key_v1(&shared, &aad);
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_slice())
        .map_err(|_| StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    let nonce: &XNonce = nonce_bytes
        .as_slice()
        .try_into()
        .map_err(|_| StateTransitionErrorV1::EncryptedOpeningMismatch)?;
    let mut plaintext = Zeroizing::new(
        cipher
            .decrypt(
                nonce,
                Payload {
                    msg: authenticated,
                    aad: aad.as_slice(),
                },
            )
            .map_err(|_| StateTransitionErrorV1::EncryptedOpeningMismatch)?,
    );
    decode_plaintext_v1(&mut plaintext)
}

fn sender_binding_v1(
    plan: &SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
) -> Result<CreditCipherBindingV1, StateTransitionErrorV1> {
    request
        .validate()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    let request_digest = request
        .canonical_digest()
        .map_err(|_| StateTransitionErrorV1::InvalidRequest)?;
    let reconstructed = plan
        .statement
        .clone()
        .seal_transition()
        .map_err(|_| StateTransitionErrorV1::CorruptPlan)?;
    if request_digest != plan.request_digest
        || reconstructed != plan.statement
        || request.release_id != plan.context.release_id
        || request.network_id != plan.context.network_id
        || request.asset != plan.context.asset
        || request.scale != plan.context.scale
        || request.amount != plan.amount
        || request.receiver_balance_commitment != plan.receiver_head
        || request.recipient_key_reference != plan.recipient_key_reference
        || plan.statement.request_digest != plan.request_digest
        || plan.statement.receiver_before != plan.receiver_head
        || plan.statement.credit_commitment != plan.credit_commitment
    {
        return Err(StateTransitionErrorV1::CorruptPlan);
    }
    CreditCipherBindingV1 {
        release_id: plan.context.release_id,
        request_digest: plan.request_digest,
        transition_digest: plan.statement.transition_digest,
        credit_commitment: plan.credit_commitment,
        recipient_key_reference: plan.recipient_key_reference,
        recipient_encryption_public_key: request.recipient_encryption_public_key,
    }
    .validate()
}

/// Encrypt the exact private credit opening owned by one prepared sender plan.
pub(crate) fn encrypt_send_split_credit_v1(
    rng: &mut (impl CryptoRng + RngCore),
    plan: &SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
) -> Result<Vec<u8>, StateTransitionErrorV1> {
    let binding = sender_binding_v1(plan, request)?;
    encrypt_credit_opening_v1(rng, binding, &plan.credit_opening)
}

/// Encrypt with operating-system entropy while preserving the same fixed codec.
pub(crate) fn encrypt_send_split_credit_with_os_rng_v1(
    plan: &SendSplitPlanV1,
    request: &OfflineCashPaymentRequestV1,
) -> Result<Vec<u8>, StateTransitionErrorV1> {
    encrypt_send_split_credit_v1(&mut rand_core_06::OsRng, plan, request)
}

fn receiver_binding_v1(
    pending: &PendingOwnerV1,
    statement: &OfflineCashTransferStatementV1,
) -> Result<CreditCipherBindingV1, StateTransitionErrorV1> {
    if statement.validate().is_err()
        || statement.release_id != pending.context.release_id
        || statement.network_id != pending.context.network_id
        || statement.asset != pending.context.asset
        || statement.scale != pending.context.scale
        || statement.amount != pending.amount
        || statement.request_digest != pending.request_digest
        || statement.receiver_before != pending.receiver_head
        || offline_cash_receiver_key_reference_v1(
            &pending.receiver_public_key,
            pending.recipient_encryption_public_key,
        ) != pending.recipient_key_reference
    {
        return Err(StateTransitionErrorV1::RequestMismatch);
    }
    CreditCipherBindingV1 {
        release_id: pending.context.release_id,
        request_digest: pending.request_digest,
        transition_digest: statement.transition_digest,
        credit_commitment: statement.credit_commitment,
        recipient_key_reference: pending.recipient_key_reference,
        recipient_encryption_public_key: pending.recipient_encryption_public_key,
    }
    .validate()
}

/// Authenticate and decrypt one proof-bound credit into a move-only opening owner.
pub(crate) fn decrypt_received_credit_v1(
    pending: &PendingOwnerV1,
    statement: &OfflineCashTransferStatementV1,
    encrypted_credit: &[u8],
    recipient_secret_key: &Digest,
) -> Result<DecryptedCreditOpeningOwnerV1, StateTransitionErrorV1> {
    let binding = receiver_binding_v1(pending, statement)?;
    let opening = decrypt_credit_opening_v1(binding, encrypted_credit, recipient_secret_key)?;
    DecryptedCreditOpeningOwnerV1::from_authenticated_decryption(
        opening,
        encrypted_credit,
        binding.recipient_key_reference,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HealthyRng {
        offset: u8,
    }

    impl RngCore for HealthyRng {
        fn next_u32(&mut self) -> u32 {
            panic!("credit encryption must use fallible bulk entropy")
        }

        fn next_u64(&mut self) -> u64 {
            panic!("credit encryption must use fallible bulk entropy")
        }

        fn fill_bytes(&mut self, _destination: &mut [u8]) {
            panic!("credit encryption must use fallible bulk entropy")
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = u8::try_from(index)
                    .unwrap_or(0)
                    .wrapping_mul(41)
                    .wrapping_add(3)
                    .wrapping_add(self.offset);
            }
            self.offset = self.offset.wrapping_add(17);
            Ok(())
        }
    }

    impl CryptoRng for HealthyRng {}

    fn binding(recipient_public_key: Digest) -> CreditCipherBindingV1 {
        CreditCipherBindingV1 {
            release_id: [1; 32],
            request_digest: [2; 32],
            transition_digest: [3; 32],
            credit_commitment: [4; 32],
            recipient_key_reference: [5; 32],
            recipient_encryption_public_key: recipient_public_key,
        }
    }

    #[test]
    fn fixed_credit_envelope_roundtrips_and_binds_every_authority() {
        let recipient_secret = [0x42; 32];
        let recipient_public = x25519_public_key_v1(&recipient_secret).expect("recipient key");
        let opening = [0x61; 32];
        let mut rng = HealthyRng { offset: 0 };
        let encrypted = encrypt_credit_opening_v1(&mut rng, binding(recipient_public), &opening)
            .expect("encrypt fixed credit");
        assert_eq!(encrypted.len(), OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES_V1);
        assert_eq!(
            *decrypt_credit_opening_v1(binding(recipient_public), &encrypted, &recipient_secret,)
                .expect("decrypt fixed credit"),
            opening
        );

        for mutate in 0..5 {
            let mut changed = binding(recipient_public);
            match mutate {
                0 => changed.release_id[0] ^= 1,
                1 => changed.request_digest[0] ^= 1,
                2 => changed.transition_digest[0] ^= 1,
                3 => changed.credit_commitment[0] ^= 1,
                _ => changed.recipient_key_reference[0] ^= 1,
            }
            assert!(
                decrypt_credit_opening_v1(changed, &encrypted, &recipient_secret).is_err(),
                "AAD substitution {mutate} must fail authentication"
            );
        }
        let mut tampered = encrypted.clone();
        *tampered.last_mut().expect("authentication tag") ^= 1;
        assert!(
            decrypt_credit_opening_v1(binding(recipient_public), &tampered, &recipient_secret,)
                .is_err()
        );
        assert!(
            decrypt_credit_opening_v1(binding(recipient_public), &encrypted, &[0x43; 32],).is_err()
        );
    }

    #[test]
    fn credit_envelope_rejects_aliases_lengths_and_zero_openings() {
        let recipient_secret = [0x42; 32];
        let recipient_public = x25519_public_key_v1(&recipient_secret).expect("recipient key");
        let mut rng = HealthyRng { offset: 0 };
        assert!(encrypt_credit_opening_v1(&mut rng, binding(recipient_public), &[0; 32],).is_err());
        assert!(binding([0; 32]).validate().is_err());
        assert!(
            decrypt_credit_opening_v1(
                binding(recipient_public),
                &[0; OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES_V1 - 1],
                &recipient_secret,
            )
            .is_err()
        );
    }
}
