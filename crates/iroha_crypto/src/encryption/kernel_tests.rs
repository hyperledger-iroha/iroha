//! Every encryptor method runs the crate's cipher kernels and keeps the library's AEAD output.
use super::*;
use aead::{Aead, Payload};

const KEY: [u8; 32] = [0x36; 32];
const NONCE: [u8; 12] = [0x51; 12];
const AAD: &[u8] = b"kernel equivalence";
// Empty, sub-block, exact ChaCha20 block and multi-block messages.
const LENGTHS: [usize; 7] = [0, 1, 63, 64, 65, 4096, 70_001];

fn encryptor() -> SymmetricEncryptor<ChaCha20Poly1305> {
    SymmetricEncryptor::new_with_key(KEY).unwrap()
}

fn message(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect()
}

/// `ciphertext || tag` from the AEAD library's own generic entry point.
fn reference(nonce: &[u8], plaintext: &[u8]) -> Vec<u8> {
    ChaCha20Poly1305::new_from_slice(&KEY)
        .unwrap()
        .encrypt(
            &nonce_from_slice::<ChaCha20Poly1305>(nonce).unwrap(),
            Payload {
                msg: plaintext,
                aad: AAD,
            },
        )
        .unwrap()
}

#[test]
fn cipher_kernels_seal_and_open_exactly_the_library_aead() {
    let cipher = ChaCha20Poly1305::new_from_slice(&KEY).unwrap();
    let nonce = nonce_from_slice::<ChaCha20Poly1305>(&NONCE).unwrap();
    for length in LENGTHS {
        let plaintext = message(length);
        let expected = reference(&NONCE, &plaintext);
        let mut sealed = plaintext.clone();
        let tag = cipher.seal_detached(&nonce, AAD, &mut sealed).unwrap();
        assert_eq!(sealed, expected[..length]);
        assert_eq!(tag.as_slice(), &expected[length..]);

        let mut opened = sealed.clone();
        cipher
            .open_detached(&nonce, AAD, &mut opened, &tag)
            .unwrap();
        assert_eq!(opened, plaintext);

        // A changed tag, associated data or message is refused.
        let mut wrong_tag = tag.as_slice().to_vec();
        wrong_tag[0] ^= 1;
        let wrong_tag = aead::Tag::<ChaCha20Poly1305>::try_from(wrong_tag.as_slice()).unwrap();
        assert!(
            cipher
                .open_detached(&nonce, AAD, &mut sealed.clone(), &wrong_tag)
                .is_err()
        );
        assert!(
            cipher
                .open_detached(&nonce, b"other", &mut sealed.clone(), &tag)
                .is_err()
        );
        if let Some(first) = sealed.first_mut() {
            *first ^= 1;
            assert!(
                cipher
                    .open_detached(&nonce, AAD, &mut sealed, &tag)
                    .is_err()
            );
        }
    }
}

#[test]
fn explicit_nonce_methods_keep_the_library_layout() {
    let encryptor = encryptor();
    for length in LENGTHS {
        let plaintext = message(length);
        let expected = reference(&NONCE, &plaintext);
        let sealed = encryptor
            .encrypt(&NONCE[..], AAD, plaintext.as_slice())
            .unwrap();
        assert_eq!(sealed, expected);
        assert_eq!(
            encryptor
                .decrypt(&NONCE[..], AAD, sealed.as_slice())
                .unwrap(),
            plaintext
        );
    }
    // A wrong nonce length is not a nonce; a body without room for its tag fails authentication.
    assert!(matches!(
        encryptor.encrypt(&NONCE[..11], AAD, &b"message"[..]),
        Err(Error::NotEnoughData)
    ));
    assert!(matches!(
        encryptor.decrypt(&NONCE[..], AAD, &[0; 15][..]),
        Err(Error::Decryption(_))
    ));
}

#[test]
fn easy_envelopes_are_the_nonce_followed_by_the_library_ciphertext() {
    let encryptor = encryptor();
    let mut sealed_into = Vec::new();
    let mut opened_into = Vec::new();
    // Descending lengths also reuse a longer buffer for a shorter envelope.
    for length in LENGTHS.into_iter().rev() {
        let plaintext = message(length);
        for envelope in [
            encryptor.encrypt_easy(AAD, plaintext.as_slice()).unwrap(),
            encryptor
                .encrypt_easy_into(AAD, plaintext.as_slice(), &mut sealed_into)
                .unwrap()
                .to_vec(),
        ] {
            assert_eq!(envelope.len(), 12 + length + 16);
            assert_eq!(envelope[12..], reference(&envelope[..12], &plaintext));
            assert_eq!(
                encryptor.decrypt_easy(AAD, envelope.as_slice()).unwrap(),
                plaintext
            );
            assert_eq!(
                encryptor
                    .decrypt_easy_into(AAD, envelope.as_slice(), &mut opened_into)
                    .unwrap(),
                plaintext
            );
            let mut in_place = envelope.clone();
            assert_eq!(
                encryptor.decrypt_easy_in_place(AAD, &mut in_place).unwrap(),
                plaintext
            );
            let mut tampered = envelope.clone();
            *tampered.last_mut().unwrap() ^= 1;
            assert!(matches!(
                encryptor.decrypt_easy(AAD, tampered.as_slice()),
                Err(Error::Decryption(_))
            ));
            assert!(matches!(
                encryptor.decrypt_easy_into(AAD, tampered.as_slice(), &mut opened_into),
                Err(Error::Decryption(_))
            ));
        }
    }
    // An envelope needs its nonce and tag.
    assert!(matches!(
        encryptor.decrypt_easy(AAD, &[0; 27][..]),
        Err(Error::NotEnoughData)
    ));
    assert!(matches!(
        encryptor.decrypt_easy_into(AAD, &[0; 27][..], &mut opened_into),
        Err(Error::NotEnoughData)
    ));
}

#[test]
fn envelope_slots_follow_the_tag_position_and_refuse_short_storage() {
    let mut body = [0u8; 20];
    let (message, tag) = split_tag::<ChaCha20Poly1305>(&mut body).unwrap();
    assert_eq!((message.len(), tag.len()), (4, 16));
    // ChaCha20-Poly1305 appends its tag.
    message.fill(1);
    assert_eq!(body[..4], [1; 4]);
    assert_eq!(body[4..], [0; 16]);
    let (message, tag) = split_tag::<ChaCha20Poly1305>(&mut body[..16]).unwrap();
    assert_eq!((message.len(), tag.len()), (0, 16));
    assert!(split_tag::<ChaCha20Poly1305>(&mut body[..15]).is_none());

    // Sealing writes nothing into storage that cannot hold the nonce and tag slots.
    let encryptor = encryptor();
    let nonce = nonce_from_slice::<ChaCha20Poly1305>(&NONCE).unwrap();
    for length in [0, 11, 12, 27] {
        let mut short = vec![0x71; length];
        assert!(matches!(
            encryptor.seal_envelope(&nonce, AAD, &mut short),
            Err(Error::NotEnoughData)
        ));
        assert_eq!(short, vec![0x71; length]);
    }
    let mut envelope = [0x71; 28];
    encryptor.seal_envelope(&nonce, AAD, &mut envelope).unwrap();
    assert_eq!(envelope[..12], NONCE);
    assert_eq!(envelope[12..], reference(&NONCE, b""));
}
