//! Typed Kagemusha V1 credit-envelope encryption.
//!
//! These helpers canonicalize the data-model plaintext and associated data,
//! then invoke the reviewed X25519/HKDF-SHA256/XChaCha20-Poly1305 primitive in
//! `iroha_crypto`. They are an implementation component for a completely
//! qualified non-forking hardware provider, not a software fallback: AEAD
//! success grants no monetary authority and never substitutes for a released
//! recursive proof, hardware transition certificate, journal, counter, inbox,
//! or outbox decision. No AEAD or X25519 arithmetic is placed in the recursive
//! circuits.

use iroha_crypto::kagemusha::{
    KagemushaCreditCryptoErrorV1, kagemusha_x25519_public_key_v1, open_kagemusha_credit_bytes_v1,
    seal_kagemusha_credit_bytes_v1,
};
use iroha_data_model::kagemusha::{
    KagemushaCreditOpeningV1, KagemushaEncryptedCreditAadV1, KagemushaEncryptedCreditEnvelopeV1,
    kagemusha_encrypted_credit_kdf_info_v1, kagemusha_encrypted_credit_kdf_salt_v1,
};
use rand::rand_core::TryCryptoRng;
use thiserror::Error;
use zeroize::Zeroizing;

/// Failure sealing or opening a typed Kagemusha V1 credit envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum KagemushaCreditEncryptionErrorV1 {
    /// The private opening was malformed or disagreed with public credit fields.
    #[error("invalid Kagemusha V1 credit opening")]
    InvalidOpening,
    /// The authenticated public credit context was malformed.
    #[error("invalid Kagemusha V1 encrypted-credit associated data")]
    InvalidAssociatedData,
    /// The signed recipient X25519 key was malformed or low-order.
    #[error("invalid Kagemusha V1 encrypted-credit recipient key")]
    InvalidRecipientKey,
    /// The supplied envelope was malformed, oversized, or non-canonical.
    #[error("invalid Kagemusha V1 encrypted-credit envelope")]
    InvalidEnvelope,
    /// A recipient private key did not project to the signed recipient key.
    #[error("Kagemusha V1 encrypted-credit recipient key mismatch")]
    RecipientKeyMismatch,
    /// The provider RNG failed or returned an all-zero ephemeral secret.
    #[error("Kagemusha V1 encrypted-credit randomness is unavailable")]
    RandomnessUnavailable,
    /// Checked key agreement, KDF, or authenticated encryption failed.
    #[error("Kagemusha V1 encrypted-credit cryptographic operation failed: {0}")]
    CryptographicFailure(KagemushaCreditCryptoErrorV1),
}

/// Seal one typed credit opening using injected provider entropy.
///
/// The RNG supplies exactly one fresh 32-byte X25519 ephemeral secret followed
/// by one fresh 24-byte XChaCha20-Poly1305 nonce. This explicit injection is for
/// hardware adapters, deterministic qualification vectors, and crash-recovery
/// reproduction of an already reserved transition; production callers must not
/// replace the qualified provider with a host software RNG.
///
/// # Errors
///
/// Returns a typed failure for invalid public/private bindings, unavailable
/// entropy, invalid key material, or failed authenticated encryption.
pub fn seal_kagemusha_credit_v1_with_rng<R: TryCryptoRng + ?Sized>(
    opening: &KagemushaCreditOpeningV1,
    aad: &KagemushaEncryptedCreditAadV1,
    recipient_x25519_public_key: [u8; 32],
    rng: &mut R,
) -> Result<KagemushaEncryptedCreditEnvelopeV1, KagemushaCreditEncryptionErrorV1> {
    opening
        .validate_shape_against(aad.credit_id, aad.amount)
        .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidOpening)?;
    let canonical_aad = aad
        .canonical_bytes()
        .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidAssociatedData)?;
    let canonical_plaintext = Zeroizing::new(
        opening
            .canonical_bytes()
            .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidOpening)?,
    );

    let mut ephemeral_private_key = Zeroizing::new([0_u8; 32]);
    fill_provider_entropy(rng, ephemeral_private_key.as_mut())?;
    if ephemeral_private_key.iter().all(|byte| *byte == 0) {
        return Err(KagemushaCreditEncryptionErrorV1::RandomnessUnavailable);
    }
    let mut nonce = [0_u8; 24];
    fill_provider_entropy(rng, &mut nonce)?;

    let ephemeral_public_key = kagemusha_x25519_public_key_v1(&ephemeral_private_key)
        .map_err(KagemushaCreditEncryptionErrorV1::CryptographicFailure)?;
    let kdf_salt =
        kagemusha_encrypted_credit_kdf_salt_v1(recipient_x25519_public_key, ephemeral_public_key)
            .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidRecipientKey)?;
    let kdf_info = kagemusha_encrypted_credit_kdf_info_v1(aad)
        .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidAssociatedData)?;
    let ciphertext = seal_kagemusha_credit_bytes_v1(
        recipient_x25519_public_key,
        &ephemeral_private_key,
        &nonce,
        &kdf_salt,
        &kdf_info,
        canonical_plaintext.as_slice(),
        &canonical_aad,
    )
    .map_err(KagemushaCreditEncryptionErrorV1::CryptographicFailure)?;
    if ciphertext.ephemeral_public_key != ephemeral_public_key {
        return Err(KagemushaCreditEncryptionErrorV1::CryptographicFailure(
            KagemushaCreditCryptoErrorV1::SealFailed,
        ));
    }
    let envelope = KagemushaEncryptedCreditEnvelopeV1 {
        version: aad.version,
        ephemeral_x25519_public_key: ciphertext.ephemeral_public_key,
        nonce,
        ciphertext_and_tag: ciphertext.ciphertext_and_tag,
    };
    envelope
        .validate_shape_against_recipient_key(recipient_x25519_public_key)
        .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidEnvelope)?;
    Ok(envelope)
}

/// Authenticate, canonically decode, and publicly bind one credit opening.
///
/// The recipient secret is borrowed so a provider can source it from a
/// non-exportable key operation. Every temporary DH secret, AEAD key, and
/// plaintext buffer created below the provider boundary is zeroized on drop.
/// The returned typed opening must remain inside that same trusted boundary.
///
/// # Errors
///
/// Returns a typed failure for invalid wire/context, recipient-key mismatch,
/// failed authenticated encryption, non-canonical plaintext, or a public
/// `credit_id`/`amount` mismatch.
pub fn open_kagemusha_credit_v1(
    envelope: &KagemushaEncryptedCreditEnvelopeV1,
    aad: &KagemushaEncryptedCreditAadV1,
    recipient_x25519_public_key: [u8; 32],
    recipient_x25519_private_key: &[u8; 32],
) -> Result<KagemushaCreditOpeningV1, KagemushaCreditEncryptionErrorV1> {
    let canonical_aad = aad
        .canonical_bytes()
        .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidAssociatedData)?;
    envelope
        .validate_shape_against_recipient_key(recipient_x25519_public_key)
        .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidEnvelope)?;
    let derived_recipient_public_key = kagemusha_x25519_public_key_v1(recipient_x25519_private_key)
        .map_err(KagemushaCreditEncryptionErrorV1::CryptographicFailure)?;
    if derived_recipient_public_key != recipient_x25519_public_key {
        return Err(KagemushaCreditEncryptionErrorV1::RecipientKeyMismatch);
    }
    let kdf_salt = envelope
        .kdf_salt_against_recipient_key(recipient_x25519_public_key)
        .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidEnvelope)?;
    let kdf_info = kagemusha_encrypted_credit_kdf_info_v1(aad)
        .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidAssociatedData)?;
    let canonical_plaintext = open_kagemusha_credit_bytes_v1(
        recipient_x25519_private_key,
        envelope.ephemeral_x25519_public_key,
        &envelope.nonce,
        &kdf_salt,
        &kdf_info,
        &envelope.ciphertext_and_tag,
        &canonical_aad,
    )
    .map_err(KagemushaCreditEncryptionErrorV1::CryptographicFailure)?;
    KagemushaCreditOpeningV1::decode_canonical_shape_exact_against(
        canonical_plaintext.as_slice(),
        aad.credit_id,
        aad.amount,
    )
    .map_err(|_| KagemushaCreditEncryptionErrorV1::InvalidOpening)
}

fn fill_provider_entropy<R: TryCryptoRng + ?Sized>(
    rng: &mut R,
    destination: &mut [u8],
) -> Result<(), KagemushaCreditEncryptionErrorV1> {
    rng.try_fill_bytes(destination)
        .map_err(|_| KagemushaCreditEncryptionErrorV1::RandomnessUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::kagemusha::KagemushaCreditCiphertextV1;
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaEncryptedCreditPurposeV1,
    };
    use rand::rand_core::{TryCryptoRng, TryRngCore};

    const EPHEMERAL_PRIVATE: [u8; 32] = [
        0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d, 0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2, 0x66,
        0x45, 0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a, 0xb1, 0x77, 0xfb, 0xa5, 0x1d, 0xb9,
        0x2c, 0x2a,
    ];
    const EPHEMERAL_PUBLIC: [u8; 32] = [
        0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e, 0xf7,
        0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e, 0xaa, 0x9b,
        0x4e, 0x6a,
    ];
    const RECIPIENT_PRIVATE: [u8; 32] = [
        0x5d, 0xab, 0x08, 0x7e, 0x62, 0x4a, 0x8a, 0x4b, 0x79, 0xe1, 0x7f, 0x8b, 0x83, 0x80, 0x0e,
        0xe6, 0x6f, 0x3b, 0xb1, 0x29, 0x26, 0x18, 0xb6, 0xfd, 0x1c, 0x2f, 0x8b, 0x27, 0xff, 0x88,
        0xe0, 0xeb,
    ];
    const RECIPIENT_PUBLIC: [u8; 32] = [
        0xde, 0x9e, 0xdb, 0x7d, 0x7b, 0x7d, 0xc1, 0xb4, 0xd3, 0x5b, 0x61, 0xc2, 0xec, 0xe4, 0x35,
        0x37, 0x3f, 0x83, 0x43, 0xc8, 0x5b, 0x78, 0x67, 0x4d, 0xad, 0xfc, 0x7e, 0x14, 0x6f, 0x88,
        0x2b, 0x4f,
    ];
    // Public RFC 7748 keys with independently reconstructed canonical Norito frames,
    // HKDF-SHA256, and XChaCha20-Poly1305. Pin the plaintext and AAD too so a future
    // schema/layout change fails before appearing as an unexplained ciphertext drift.
    const OPENING_KAT_HEX: &str = concat!(
        "4e52543000003283089e9d5f5b1495de9c20d961267b00980000000000000011",
        "b2ecacb32f2d7a02000000000000000002010020111111111111111111111111",
        "1111111111111111111111111111111111111111102500000000000000000000",
        "0000000000202222222222222222222222222222222222222222222222222222",
        "2222222222222033333333333333333333333333333333333333333333333333",
        "3333333333333320444444444444444444444444444444444444444444444444",
        "4444444444444444",
    );
    const AAD_KAT_HEX: &str = concat!(
        "4e5254300000ccbd3324c0c9da3aff2aab04ec3f24f8007c00000000000000fa",
        "cc47d455bc45d602000000000000000002010004010000002055555555555555",
        "5555555555555555555555555555555555555555555555555520666666666666",
        "6666666666666666666666666666666666666666666666666666201111111111",
        "1111111111111111111111111111111111111111111111111111111025000000",
        "000000000000000000000000",
    );
    const TYPED_ENVELOPE_KAT_HEX: &str = concat!(
        "4e5254300000340212827b285b0cef93dc4763964909001f0100000000000075",
        "99d14818dbee5802020100208520f0098930a754748b7ddcb43ef75a0dbf3a0d",
        "26381af4eba4a98eaa9b4e6a18a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5",
        "a5a5a5a5a5e001d800000000000000025a6fb318d7e6d6e47362d71dd6f44c20",
        "ef17888f1768c8ec4744031893213d63d2dd5752685bdbc8aefcf6404fdb0e9c",
        "74538d7e01c530a002bddb5d67040b20198ed87ad442006f93ba0c96a08f79bb",
        "8167dc4819c57cb3f3b2b49060a30e5b8c31e04976e46a84c5782de362217fc1",
        "3fb2b589900f09685966643a4d374127f7c0fbaa4354528e679ca2feff099b02",
        "8f18f8739eb8c1bf83e093e6ab5f47a0e0ebcee2ff9120758476acab05adaae0",
        "3ce749038f2e4133975bf288f16002dfad12d9be359272407714d754d111a467",
        "3d4534114440ca",
    );

    fn opening() -> KagemushaCreditOpeningV1 {
        KagemushaCreditOpeningV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            credit_id: [0x11; 32],
            amount: 37,
            credit_commitment_opening: [0x22; 32],
            recipient_binding_opening: [0x33; 32],
            recovery_nonce: [0x44; 32],
        }
    }

    fn aad() -> KagemushaEncryptedCreditAadV1 {
        KagemushaEncryptedCreditAadV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            purpose: KagemushaEncryptedCreditPurposeV1::Peer,
            context_digest: [0x55; 32],
            issuance_or_transition_commitment: [0x66; 32],
            credit_id: [0x11; 32],
            amount: 37,
        }
    }

    #[derive(Clone)]
    struct FixedEntropy {
        bytes: [u8; 56],
        offset: usize,
    }

    impl FixedEntropy {
        fn kat() -> Self {
            let mut bytes = [0_u8; 56];
            bytes[..32].copy_from_slice(&EPHEMERAL_PRIVATE);
            bytes[32..].fill(0xA5);
            Self { bytes, offset: 0 }
        }
    }

    #[derive(Debug)]
    struct FixedEntropyExhausted;

    impl core::fmt::Display for FixedEntropyExhausted {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("fixed Kagemusha entropy exhausted")
        }
    }

    impl TryRngCore for FixedEntropy {
        type Error = FixedEntropyExhausted;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            let mut bytes = [0_u8; 4];
            self.try_fill_bytes(&mut bytes)?;
            Ok(u32::from_le_bytes(bytes))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            let mut bytes = [0_u8; 8];
            self.try_fill_bytes(&mut bytes)?;
            Ok(u64::from_le_bytes(bytes))
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            let end = self
                .offset
                .checked_add(destination.len())
                .ok_or(FixedEntropyExhausted)?;
            let source = self
                .bytes
                .get(self.offset..end)
                .ok_or(FixedEntropyExhausted)?;
            destination.copy_from_slice(source);
            self.offset = end;
            Ok(())
        }
    }

    impl TryCryptoRng for FixedEntropy {}

    #[test]
    fn injected_entropy_is_deterministic_and_roundtrips() {
        let opening = opening();
        let aad = aad();
        assert_eq!(
            hex::encode(
                opening
                    .canonical_bytes()
                    .expect("canonical fixture opening")
            ),
            OPENING_KAT_HEX
        );
        assert_eq!(
            hex::encode(aad.canonical_bytes().expect("canonical fixture AAD")),
            AAD_KAT_HEX
        );
        let envelope = seal_kagemusha_credit_v1_with_rng(
            &opening,
            &aad,
            RECIPIENT_PUBLIC,
            &mut FixedEntropy::kat(),
        )
        .expect("seal deterministic envelope");
        let repeated = seal_kagemusha_credit_v1_with_rng(
            &opening,
            &aad,
            RECIPIENT_PUBLIC,
            &mut FixedEntropy::kat(),
        )
        .expect("repeat deterministic envelope");
        assert_eq!(envelope, repeated);
        assert_eq!(envelope.ephemeral_x25519_public_key, EPHEMERAL_PUBLIC);
        assert_eq!(envelope.nonce, [0xA5; 24]);
        assert_eq!(
            hex::encode(
                envelope
                    .canonical_bytes_against_recipient_key(RECIPIENT_PUBLIC)
                    .expect("canonical typed envelope")
            ),
            TYPED_ENVELOPE_KAT_HEX
        );
        assert_eq!(
            open_kagemusha_credit_v1(&envelope, &aad, RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,)
                .expect("open deterministic envelope"),
            opening
        );
    }

    #[test]
    fn tamper_wrong_aad_and_wrong_key_fail_closed() {
        let opening = opening();
        let aad = aad();
        let envelope = seal_kagemusha_credit_v1_with_rng(
            &opening,
            &aad,
            RECIPIENT_PUBLIC,
            &mut FixedEntropy::kat(),
        )
        .expect("seal");

        let mut tampered = envelope.clone();
        tampered.ciphertext_and_tag[0] ^= 1;
        assert!(matches!(
            open_kagemusha_credit_v1(&tampered, &aad, RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,),
            Err(KagemushaCreditEncryptionErrorV1::CryptographicFailure(
                KagemushaCreditCryptoErrorV1::OpenFailed
            ))
        ));

        let mut wrong_aad = aad;
        wrong_aad.context_digest[0] ^= 1;
        assert!(matches!(
            open_kagemusha_credit_v1(&envelope, &wrong_aad, RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,),
            Err(KagemushaCreditEncryptionErrorV1::CryptographicFailure(
                KagemushaCreditCryptoErrorV1::OpenFailed
            ))
        ));

        assert_eq!(
            open_kagemusha_credit_v1(&envelope, &aad, RECIPIENT_PUBLIC, &[0x77; 32],),
            Err(KagemushaCreditEncryptionErrorV1::RecipientKeyMismatch)
        );
    }

    #[test]
    fn authenticated_public_credit_mismatch_is_rejected_after_open() {
        let aad = aad();
        let mut substituted_opening = opening();
        substituted_opening.credit_id = [0x99; 32];
        let plaintext = Zeroizing::new(
            substituted_opening
                .canonical_bytes()
                .expect("canonical substituted opening"),
        );
        let aad_bytes = aad.canonical_bytes().expect("canonical aad");
        let kdf_salt = kagemusha_encrypted_credit_kdf_salt_v1(RECIPIENT_PUBLIC, EPHEMERAL_PUBLIC)
            .expect("kdf salt");
        let kdf_info = kagemusha_encrypted_credit_kdf_info_v1(&aad).expect("kdf info");
        let KagemushaCreditCiphertextV1 {
            ephemeral_public_key,
            ciphertext_and_tag,
        } = seal_kagemusha_credit_bytes_v1(
            RECIPIENT_PUBLIC,
            &EPHEMERAL_PRIVATE,
            &[0xA5; 24],
            &kdf_salt,
            &kdf_info,
            plaintext.as_slice(),
            &aad_bytes,
        )
        .expect("seal adversarial plaintext");
        let envelope = KagemushaEncryptedCreditEnvelopeV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            ephemeral_x25519_public_key: ephemeral_public_key,
            nonce: [0xA5; 24],
            ciphertext_and_tag,
        };
        assert_eq!(
            open_kagemusha_credit_v1(&envelope, &aad, RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,),
            Err(KagemushaCreditEncryptionErrorV1::InvalidOpening)
        );
    }

    #[test]
    fn zero_ephemeral_secret_and_low_order_recipient_are_rejected() {
        let mut zero_entropy = FixedEntropy {
            bytes: [0; 56],
            offset: 0,
        };
        assert_eq!(
            seal_kagemusha_credit_v1_with_rng(
                &opening(),
                &aad(),
                RECIPIENT_PUBLIC,
                &mut zero_entropy,
            ),
            Err(KagemushaCreditEncryptionErrorV1::RandomnessUnavailable)
        );
        assert_eq!(
            seal_kagemusha_credit_v1_with_rng(
                &opening(),
                &aad(),
                [0; 32],
                &mut FixedEntropy::kat(),
            ),
            Err(KagemushaCreditEncryptionErrorV1::InvalidRecipientKey)
        );
        let mut exhausted = FixedEntropy::kat();
        exhausted.offset = exhausted.bytes.len();
        assert_eq!(
            seal_kagemusha_credit_v1_with_rng(&opening(), &aad(), RECIPIENT_PUBLIC, &mut exhausted,),
            Err(KagemushaCreditEncryptionErrorV1::RandomnessUnavailable)
        );
    }

    #[test]
    fn zero_nonce_is_accepted_when_provider_state_guarantees_freshness() {
        let mut bytes = [0_u8; 56];
        bytes[..32].copy_from_slice(&EPHEMERAL_PRIVATE);
        let envelope = seal_kagemusha_credit_v1_with_rng(
            &opening(),
            &aad(),
            RECIPIENT_PUBLIC,
            &mut FixedEntropy { bytes, offset: 0 },
        )
        .expect("nonce uniqueness is a qualified-provider state property");
        assert_eq!(envelope.nonce, [0; 24]);
        assert_eq!(
            open_kagemusha_credit_v1(&envelope, &aad(), RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,)
                .expect("open zero-nonce regression"),
            opening()
        );
    }
}
