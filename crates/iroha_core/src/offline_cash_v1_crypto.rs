//! Typed Offline Cash V1 credit-envelope encryption.
//!
//! These helpers canonicalize the data-model plaintext and associated data,
//! then invoke the reviewed X25519/HKDF-SHA256/XChaCha20-Poly1305 primitive in
//! `iroha_crypto`. They are an implementation component for a completely
//! qualified non-forking hardware provider, not a software fallback: AEAD
//! success grants no monetary authority and never substitutes for a released
//! recursive proof, hardware transition certificate, journal, counter, inbox,
//! or outbox decision. No AEAD or X25519 arithmetic is placed in the recursive
//! circuits.

use iroha_crypto::offline_cash::{
    OfflineCashCreditCryptoErrorV1, offline_cash_x25519_public_key_v1,
    open_offline_cash_credit_bytes_v1, seal_offline_cash_credit_bytes_v1,
};
use iroha_data_model::offline::{
    OfflineCashCreditOpeningV1, OfflineCashEncryptedCreditAadV1,
    OfflineCashEncryptedCreditEnvelopeV1, offline_cash_encrypted_credit_kdf_info_v1,
    offline_cash_encrypted_credit_kdf_salt_v1,
};
use rand::rand_core::TryCryptoRng;
use thiserror::Error;
use zeroize::Zeroizing;

/// Failure sealing or opening a typed Offline Cash V1 credit envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum OfflineCashCreditEncryptionErrorV1 {
    /// The private opening was malformed or disagreed with public credit fields.
    #[error("invalid Offline Cash V1 credit opening")]
    InvalidOpening,
    /// The authenticated public credit context was malformed.
    #[error("invalid Offline Cash V1 encrypted-credit associated data")]
    InvalidAssociatedData,
    /// The signed recipient X25519 key was malformed or low-order.
    #[error("invalid Offline Cash V1 encrypted-credit recipient key")]
    InvalidRecipientKey,
    /// The supplied envelope was malformed, oversized, or non-canonical.
    #[error("invalid Offline Cash V1 encrypted-credit envelope")]
    InvalidEnvelope,
    /// A recipient private key did not project to the signed recipient key.
    #[error("Offline Cash V1 encrypted-credit recipient key mismatch")]
    RecipientKeyMismatch,
    /// The provider RNG failed or returned an all-zero ephemeral secret.
    #[error("Offline Cash V1 encrypted-credit randomness is unavailable")]
    RandomnessUnavailable,
    /// Checked key agreement, KDF, or authenticated encryption failed.
    #[error("Offline Cash V1 encrypted-credit cryptographic operation failed: {0}")]
    CryptographicFailure(OfflineCashCreditCryptoErrorV1),
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
pub fn seal_offline_cash_credit_v1_with_rng<R: TryCryptoRng + ?Sized>(
    opening: &OfflineCashCreditOpeningV1,
    aad: &OfflineCashEncryptedCreditAadV1,
    recipient_x25519_public_key: [u8; 32],
    rng: &mut R,
) -> Result<OfflineCashEncryptedCreditEnvelopeV1, OfflineCashCreditEncryptionErrorV1> {
    opening
        .validate_shape_against(aad.credit_id, aad.amount)
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidOpening)?;
    let canonical_aad = aad
        .canonical_bytes()
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidAssociatedData)?;
    let canonical_plaintext = Zeroizing::new(
        opening
            .canonical_bytes()
            .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidOpening)?,
    );

    let mut ephemeral_private_key = Zeroizing::new([0_u8; 32]);
    fill_provider_entropy(rng, ephemeral_private_key.as_mut())?;
    if ephemeral_private_key.iter().all(|byte| *byte == 0) {
        return Err(OfflineCashCreditEncryptionErrorV1::RandomnessUnavailable);
    }
    let mut nonce = [0_u8; 24];
    fill_provider_entropy(rng, &mut nonce)?;

    let ephemeral_public_key = offline_cash_x25519_public_key_v1(&ephemeral_private_key)
        .map_err(OfflineCashCreditEncryptionErrorV1::CryptographicFailure)?;
    let kdf_salt = offline_cash_encrypted_credit_kdf_salt_v1(
        recipient_x25519_public_key,
        ephemeral_public_key,
    )
    .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidRecipientKey)?;
    let kdf_info = offline_cash_encrypted_credit_kdf_info_v1(aad)
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidAssociatedData)?;
    let ciphertext = seal_offline_cash_credit_bytes_v1(
        recipient_x25519_public_key,
        &ephemeral_private_key,
        &nonce,
        &kdf_salt,
        &kdf_info,
        canonical_plaintext.as_slice(),
        &canonical_aad,
    )
    .map_err(OfflineCashCreditEncryptionErrorV1::CryptographicFailure)?;
    if ciphertext.ephemeral_public_key != ephemeral_public_key {
        return Err(OfflineCashCreditEncryptionErrorV1::CryptographicFailure(
            OfflineCashCreditCryptoErrorV1::SealFailed,
        ));
    }
    let envelope = OfflineCashEncryptedCreditEnvelopeV1 {
        version: aad.version,
        ephemeral_x25519_public_key: ciphertext.ephemeral_public_key,
        nonce,
        ciphertext_and_tag: ciphertext.ciphertext_and_tag,
    };
    envelope
        .validate_shape_against_recipient_key(recipient_x25519_public_key)
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidEnvelope)?;
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
pub fn open_offline_cash_credit_v1(
    envelope: &OfflineCashEncryptedCreditEnvelopeV1,
    aad: &OfflineCashEncryptedCreditAadV1,
    recipient_x25519_public_key: [u8; 32],
    recipient_x25519_private_key: &[u8; 32],
) -> Result<OfflineCashCreditOpeningV1, OfflineCashCreditEncryptionErrorV1> {
    let canonical_aad = aad
        .canonical_bytes()
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidAssociatedData)?;
    envelope
        .validate_shape_against_recipient_key(recipient_x25519_public_key)
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidEnvelope)?;
    let derived_recipient_public_key =
        offline_cash_x25519_public_key_v1(recipient_x25519_private_key)
            .map_err(OfflineCashCreditEncryptionErrorV1::CryptographicFailure)?;
    if derived_recipient_public_key != recipient_x25519_public_key {
        return Err(OfflineCashCreditEncryptionErrorV1::RecipientKeyMismatch);
    }
    let kdf_salt = envelope
        .kdf_salt_against_recipient_key(recipient_x25519_public_key)
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidEnvelope)?;
    let kdf_info = offline_cash_encrypted_credit_kdf_info_v1(aad)
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidAssociatedData)?;
    let canonical_plaintext = open_offline_cash_credit_bytes_v1(
        recipient_x25519_private_key,
        envelope.ephemeral_x25519_public_key,
        &envelope.nonce,
        &kdf_salt,
        &kdf_info,
        &envelope.ciphertext_and_tag,
        &canonical_aad,
    )
    .map_err(OfflineCashCreditEncryptionErrorV1::CryptographicFailure)?;
    OfflineCashCreditOpeningV1::decode_canonical_shape_exact_against(
        canonical_plaintext.as_slice(),
        aad.credit_id,
        aad.amount,
    )
    .map_err(|_| OfflineCashCreditEncryptionErrorV1::InvalidOpening)
}

fn fill_provider_entropy<R: TryCryptoRng + ?Sized>(
    rng: &mut R,
    destination: &mut [u8],
) -> Result<(), OfflineCashCreditEncryptionErrorV1> {
    rng.try_fill_bytes(destination)
        .map_err(|_| OfflineCashCreditEncryptionErrorV1::RandomnessUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::offline_cash::OfflineCashCreditCiphertextV1;
    use iroha_data_model::offline::{
        OFFLINE_CASH_WIRE_VERSION_V1, OfflineCashEncryptedCreditPurposeV1,
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
    const TYPED_ENVELOPE_KAT_HEX: &str = concat!(
        "4e525430000073550b5069c0fdb105ebe7e810b71b3f001f01000000000000c7",
        "59e8c2f2209cf402020100208520f0098930a754748b7ddcb43ef75a0dbf3a0d",
        "26381af4eba4a98eaa9b4e6a18a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5",
        "a5a5a5a5a5a5a5e001d80000000000000020a5da85d238bff0f1abcda157",
        "21f478de45b547b51fcc35f4b72046b940421acc8805d46c005aee52d3333e3",
        "aa29a9bebf1016b51379248bdef107a99cd821318c57714dc32a650bd9de9884",
        "da13dfd6c14a1afc1b76d0f0f770e5b564d58ad9f30aca3a000b2640686632",
        "3a13f9400e4fa372552dc2245fc03d73621f8373e211ec3c18d4bcb571c784a",
        "6c02d838fdbbd21799db09a42a469a714ee0781022529ffcf9e407896080e960",
        "a2d9e246984fc7a09f814592008674939b2a0493ee9a88beb1d5ea71dc53c00",
        "55b1250cb633381d32010106c2e",
    );

    fn opening() -> OfflineCashCreditOpeningV1 {
        OfflineCashCreditOpeningV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credit_id: [0x11; 32],
            amount: 37,
            credit_commitment_opening: [0x22; 32],
            recipient_binding_opening: [0x33; 32],
            recovery_nonce: [0x44; 32],
        }
    }

    fn aad() -> OfflineCashEncryptedCreditAadV1 {
        OfflineCashEncryptedCreditAadV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            purpose: OfflineCashEncryptedCreditPurposeV1::Peer,
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
            formatter.write_str("fixed Offline Cash entropy exhausted")
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
        let envelope = seal_offline_cash_credit_v1_with_rng(
            &opening,
            &aad,
            RECIPIENT_PUBLIC,
            &mut FixedEntropy::kat(),
        )
        .expect("seal deterministic envelope");
        let repeated = seal_offline_cash_credit_v1_with_rng(
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
            open_offline_cash_credit_v1(&envelope, &aad, RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,)
                .expect("open deterministic envelope"),
            opening
        );
    }

    #[test]
    fn shared_fixture_encrypted_credit_is_the_native_decrypting_kat() {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/offline_cash_v1.json"
        )))
        .expect("shared Offline Cash fixture");
        let fixture_bytes = |name: &str| {
            hex::decode(
                fixture
                    .get(name)
                    .and_then(|entry| entry.get("norito_hex"))
                    .and_then(norito::json::Value::as_str)
                    .expect("fixture Norito hex"),
            )
            .expect("fixture hex")
        };
        let envelope_bytes = fixture_bytes("encrypted_credit_envelope");
        let envelope =
            OfflineCashEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &envelope_bytes,
                RECIPIENT_PUBLIC,
            )
            .expect("fixture encrypted-credit envelope");
        let aad: OfflineCashEncryptedCreditAadV1 =
            norito::decode_canonical(&fixture_bytes("encrypted_credit_aad"))
                .expect("fixture encrypted-credit AAD");
        aad.validate_shape().expect("fixture AAD shape");
        let expected_opening: OfflineCashCreditOpeningV1 =
            norito::decode_canonical(&fixture_bytes("credit_opening"))
                .expect("fixture credit opening");
        expected_opening
            .validate_shape_against(aad.credit_id, aad.amount)
            .expect("fixture credit opening shape");
        assert_eq!(
            open_offline_cash_credit_v1(&envelope, &aad, RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE)
                .expect("decrypt shared fixture envelope"),
            expected_opening,
        );
        assert_eq!(hex::encode(envelope_bytes), TYPED_ENVELOPE_KAT_HEX);
    }

    #[test]
    fn tamper_wrong_aad_and_wrong_key_fail_closed() {
        let opening = opening();
        let aad = aad();
        let envelope = seal_offline_cash_credit_v1_with_rng(
            &opening,
            &aad,
            RECIPIENT_PUBLIC,
            &mut FixedEntropy::kat(),
        )
        .expect("seal");

        let mut tampered = envelope.clone();
        tampered.ciphertext_and_tag[0] ^= 1;
        assert!(matches!(
            open_offline_cash_credit_v1(&tampered, &aad, RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,),
            Err(OfflineCashCreditEncryptionErrorV1::CryptographicFailure(
                OfflineCashCreditCryptoErrorV1::OpenFailed
            ))
        ));

        let mut wrong_aad = aad;
        wrong_aad.context_digest[0] ^= 1;
        assert!(matches!(
            open_offline_cash_credit_v1(
                &envelope,
                &wrong_aad,
                RECIPIENT_PUBLIC,
                &RECIPIENT_PRIVATE,
            ),
            Err(OfflineCashCreditEncryptionErrorV1::CryptographicFailure(
                OfflineCashCreditCryptoErrorV1::OpenFailed
            ))
        ));

        assert_eq!(
            open_offline_cash_credit_v1(&envelope, &aad, RECIPIENT_PUBLIC, &[0x77; 32],),
            Err(OfflineCashCreditEncryptionErrorV1::RecipientKeyMismatch)
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
        let kdf_salt =
            offline_cash_encrypted_credit_kdf_salt_v1(RECIPIENT_PUBLIC, EPHEMERAL_PUBLIC)
                .expect("kdf salt");
        let kdf_info = offline_cash_encrypted_credit_kdf_info_v1(&aad).expect("kdf info");
        let OfflineCashCreditCiphertextV1 {
            ephemeral_public_key,
            ciphertext_and_tag,
        } = seal_offline_cash_credit_bytes_v1(
            RECIPIENT_PUBLIC,
            &EPHEMERAL_PRIVATE,
            &[0xA5; 24],
            &kdf_salt,
            &kdf_info,
            plaintext.as_slice(),
            &aad_bytes,
        )
        .expect("seal adversarial plaintext");
        let envelope = OfflineCashEncryptedCreditEnvelopeV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            ephemeral_x25519_public_key: ephemeral_public_key,
            nonce: [0xA5; 24],
            ciphertext_and_tag,
        };
        assert_eq!(
            open_offline_cash_credit_v1(&envelope, &aad, RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,),
            Err(OfflineCashCreditEncryptionErrorV1::InvalidOpening)
        );
    }

    #[test]
    fn zero_ephemeral_secret_and_low_order_recipient_are_rejected() {
        let mut zero_entropy = FixedEntropy {
            bytes: [0; 56],
            offset: 0,
        };
        assert_eq!(
            seal_offline_cash_credit_v1_with_rng(
                &opening(),
                &aad(),
                RECIPIENT_PUBLIC,
                &mut zero_entropy,
            ),
            Err(OfflineCashCreditEncryptionErrorV1::RandomnessUnavailable)
        );
        assert_eq!(
            seal_offline_cash_credit_v1_with_rng(
                &opening(),
                &aad(),
                [0; 32],
                &mut FixedEntropy::kat(),
            ),
            Err(OfflineCashCreditEncryptionErrorV1::InvalidRecipientKey)
        );
        let mut exhausted = FixedEntropy::kat();
        exhausted.offset = exhausted.bytes.len();
        assert_eq!(
            seal_offline_cash_credit_v1_with_rng(
                &opening(),
                &aad(),
                RECIPIENT_PUBLIC,
                &mut exhausted,
            ),
            Err(OfflineCashCreditEncryptionErrorV1::RandomnessUnavailable)
        );
    }

    #[test]
    fn zero_nonce_is_accepted_when_provider_state_guarantees_freshness() {
        let mut bytes = [0_u8; 56];
        bytes[..32].copy_from_slice(&EPHEMERAL_PRIVATE);
        let envelope = seal_offline_cash_credit_v1_with_rng(
            &opening(),
            &aad(),
            RECIPIENT_PUBLIC,
            &mut FixedEntropy { bytes, offset: 0 },
        )
        .expect("nonce uniqueness is a qualified-provider state property");
        assert_eq!(envelope.nonce, [0; 24]);
        assert_eq!(
            open_offline_cash_credit_v1(&envelope, &aad(), RECIPIENT_PUBLIC, &RECIPIENT_PRIVATE,)
                .expect("open zero-nonce regression"),
            opening()
        );
    }
}
