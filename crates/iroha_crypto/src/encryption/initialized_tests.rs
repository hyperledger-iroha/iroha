//! Fixed initialized envelope equivalence and refusal controls.
use super::*;
use crate::test_allocations::without_allocations;

struct NonceRng {
    byte: u8,
    draws: usize,
}
impl TryRngCore for NonceRng {
    type Error = OsError;
    fn try_next_u32(&mut self) -> Result<u32, OsError> {
        unreachable!()
    }
    fn try_next_u64(&mut self) -> Result<u64, OsError> {
        unreachable!()
    }
    fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), OsError> {
        self.draws += 1;
        bytes.fill(self.byte);
        Ok(())
    }
}

#[test]
fn initialized_envelope_matches_original_ciphertext_and_authenticates_without_allocation() {
    let cipher = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key([0x36; 32]).unwrap();
    let plaintext = [0x72; 96];
    let aad = b"exact initialized DKG edge";
    let nonce = [0x51; 12];
    let original = cipher
        .encrypt(&nonce[..], &aad[..], &plaintext[..])
        .unwrap();
    let mut envelope = zeroize::Zeroizing::new([0; 124]);
    envelope[12..108].copy_from_slice(&plaintext);
    let mut rng = NonceRng {
        byte: 0x51,
        draws: 0,
    };
    without_allocations(|| {
        cipher
            .encrypt_easy_in_place_from_rng(aad, &mut envelope[..], &mut rng)
            .unwrap()
    });
    assert_eq!(rng.draws, 1);
    assert_eq!(&envelope[..12], &nonce);
    assert_eq!(&envelope[12..], &original);
    let mut tampered = envelope.clone();
    tampered[123] ^= 1;
    assert!(matches!(
        without_allocations(|| cipher
            .decrypt_easy_in_place(aad, &mut tampered[..])
            .map(|_| ())),
        Err(Error::Decryption(_))
    ));
    assert!(matches!(
        without_allocations(|| cipher
            .decrypt_easy_in_place(b"wrong", &mut envelope[..])
            .map(|_| ())),
        Err(Error::Decryption(_))
    ));
    // Failed authentication may alter its buffer; retry the unchanged canonical bytes.
    envelope[..12].copy_from_slice(&nonce);
    envelope[12..].copy_from_slice(&original);
    without_allocations(|| {
        assert_eq!(
            cipher
                .decrypt_easy_in_place(aad, &mut envelope[..])
                .unwrap(),
            &plaintext
        );
    });
}

#[test]
fn initialized_envelope_refuses_short_storage_before_rng_and_inert_nonce_before_writes() {
    let cipher = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key([0x36; 32]).unwrap();
    let mut rng = NonceRng { byte: 0, draws: 0 };
    let mut short = [0x71; 27];
    assert!(matches!(
        cipher.encrypt_easy_in_place_from_rng(b"aad", &mut short, &mut rng),
        Err(Error::NotEnoughData)
    ));
    assert_eq!(rng.draws, 0);
    assert_eq!(short, [0x71; 27]);
    let mut envelope = [0x71; 124];
    assert!(matches!(
        cipher.encrypt_easy_in_place_from_rng(b"aad", &mut envelope, &mut rng),
        Err(Error::InertNonce)
    ));
    assert_eq!(rng.draws, 1);
    assert_eq!(envelope, [0x71; 124]);
}
