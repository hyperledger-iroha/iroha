//! Property-based tests for SM4 AEAD helpers.
#![cfg(all(feature = "sm", feature = "sm_proptest"))]

use iroha_crypto::Sm4Key;
use proptest::{collection::vec, prelude::*, prop_oneof};

#[path = "sm4_wycheproof_fixture.rs"]
mod sm4_wycheproof_fixture;

use sm4_wycheproof_fixture::{Sm4WycheproofMode, load_sm4_wycheproof_cases};

fn tag_length_strategy() -> impl Strategy<Value = usize> {
    prop_oneof![
        Just(4usize),
        Just(6usize),
        Just(8usize),
        Just(10usize),
        Just(12usize),
        Just(14usize),
        Just(16usize)
    ]
}

#[test]
fn sm4_wycheproof_invalid_cases_are_rejected() {
    for case in load_sm4_wycheproof_cases() {
        let key = Sm4Key::new(case.key);
        match case.mode {
            Sm4WycheproofMode::Gcm => {
                let nonce: [u8; 12] =
                    case.nonce.as_slice().try_into().unwrap_or_else(|_| {
                        panic!("GCM test {} must use 12-byte nonce", case.tc_id)
                    });
                let tag: [u8; 16] = case
                    .tag
                    .as_slice()
                    .try_into()
                    .unwrap_or_else(|_| panic!("GCM test {} must use 16-byte tag", case.tc_id));
                assert!(
                    key.decrypt_gcm(&nonce, &case.aad, &case.ciphertext, &tag)
                        .is_err(),
                    "SM4-GCM Wycheproof case {} ({}) unexpectedly succeeded",
                    case.tc_id,
                    case.comment
                );
            }
            Sm4WycheproofMode::Ccm => {
                assert!(
                    key.decrypt_ccm(&case.nonce, &case.aad, &case.ciphertext, &case.tag)
                        .is_err(),
                    "SM4-CCM Wycheproof case {} ({}) unexpectedly succeeded",
                    case.tc_id,
                    case.comment
                );
            }
        }
    }
}

proptest! {
    #[test]
    fn sm4_gcm_detects_tampering(
        key_bytes in any::<[u8; 16]>(),
        nonce_bytes in any::<[u8; 12]>(),
        aad in vec(any::<u8>(), 0..32),
        plaintext in vec(any::<u8>(), 0..64),
        flip in any::<u8>(),
    ) {
        let key = Sm4Key::new(key_bytes);
        let (ciphertext, tag) = key.encrypt_gcm(&nonce_bytes, &aad, &plaintext)
            .expect("encrypt_gcm should succeed with 16-byte key");

        // Tamper with authentication tag.
        let mut tampered_tag = tag;
        let idx = (flip as usize) % tampered_tag.len();
        tampered_tag[idx] ^= 0x01;
        prop_assert!(
            key.decrypt_gcm(&nonce_bytes, &aad, &ciphertext, &tampered_tag).is_err(),
            "SM4-GCM must reject when tag byte {} is flipped",
            idx,
        );

        if !ciphertext.is_empty() {
            let mut tampered_cipher = ciphertext.clone();
            tampered_cipher[0] ^= 0x80;
            prop_assert!(
                key.decrypt_gcm(&nonce_bytes, &aad, &tampered_cipher, &tag).is_err(),
                "SM4-GCM must reject when ciphertext is modified",
            );
        }
    }

    #[test]
    fn sm4_ccm_detects_tampering(
        key_bytes in any::<[u8; 16]>(),
        nonce in vec(any::<u8>(), 7..14),
        aad in vec(any::<u8>(), 0..32),
        plaintext in vec(any::<u8>(), 0..64),
        tag_len in tag_length_strategy(),
        flip in any::<u8>(),
    ) {
        let key = Sm4Key::new(key_bytes);
        let (ciphertext, tag) = key.encrypt_ccm(&nonce, &aad, &plaintext, tag_len)
            .expect("encrypt_ccm should succeed for supported tag lengths");

        // Tamper with ciphertext (if present).
        if !ciphertext.is_empty() {
            let mut tampered_cipher = ciphertext.clone();
            tampered_cipher[0] ^= 0x40;
            prop_assert!(
                key.decrypt_ccm(&nonce, &aad, &tampered_cipher, &tag).is_err(),
                "SM4-CCM must reject modified ciphertext",
            );
        }

        // Tamper with tag (dynamic slice).
        let mut tampered_tag = tag.clone();
        let idx = (flip as usize) % tampered_tag.len();
        tampered_tag[idx] ^= 0x02;
        prop_assert!(
            key.decrypt_ccm(&nonce, &aad, &ciphertext, &tampered_tag).is_err(),
            "SM4-CCM must reject altered tag",
        );

        if tag.len() > 1 {
            // Drop the last byte to simulate truncation.
            let truncated_tag = &tampered_tag[..tampered_tag.len() - 1];
            prop_assert!(
                key.decrypt_ccm(&nonce, &aad, &ciphertext, truncated_tag).is_err(),
                "SM4-CCM must reject truncated tags",
            );
        }
    }
}
