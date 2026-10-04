//! Fixed DKG private-edge transport with authentic purpose/session equations.
use super::*;
use crate::{hybrid::HybridKeyPair, test_allocations::without_allocations};

fn check<P: ThresholdBlsPurpose>(n: u16) {
    let parameters = dealer_generation_scratch::parameters::<P>(n);
    let mut rng = ChaCha20Rng::from_seed([0x37; 32]);
    let (secret, dealer) = DasRenDealerSecret::generate_with_rng(&parameters, 1, &mut rng).unwrap();
    let share = secret.private_share(&parameters, &dealer, n).unwrap();
    let recipient = HybridKeyPair::try_generate(&mut OsRng).unwrap();
    let aad = b"fixed DKG edge: network/generation/session/roster/dealer/recipient";
    // Warm platform entropy machinery, but measured calls still use the real RNG.
    seal_das_ren_private_share(&share, recipient.public(), aad).unwrap();
    let (kem, ciphertext) = without_allocations(|| {
        seal_das_ren_private_share(&share, recipient.public(), aad).unwrap()
    });
    let opened = without_allocations(|| {
        open_das_ren_private_share(
            &parameters,
            &dealer,
            n,
            recipient.secret(),
            &kem,
            &ciphertext,
            aad,
        )
        .unwrap()
    });
    assert_eq!(
        *opened.components_for_authenticated_encryption(),
        *share.components_for_authenticated_encryption()
    );
    assert_eq!(opened.dealer_index(), 1);
    assert_eq!(opened.recipient_index(), n);

    // Independently reproduce the old allocating AEAD using the emitted nonce
    // and the same derived key; no production randomness is replaced.
    let derived = hybrid::decapsulate(
        HybridSuite::X25519MlKem768ChaCha20Poly1305,
        &kem,
        recipient.secret(),
    )
    .unwrap();
    let key = Zeroizing::new(derived.encryption_key());
    let cipher = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(&key[..]).unwrap();
    let parts = share.components_for_authenticated_encryption();
    let mut plaintext = Zeroizing::new([0; 96]);
    for (i, component) in parts.iter().enumerate() {
        plaintext[i * 32..(i + 1) * 32].copy_from_slice(component);
    }
    let original = cipher
        .encrypt(&ciphertext[..12], &aad[..], &plaintext[..])
        .unwrap();
    assert_eq!(&ciphertext[12..], &original);
    let original_plaintext =
        Zeroizing::new(cipher.decrypt_easy(&aad[..], &ciphertext[..]).unwrap());
    assert_eq!(original_plaintext.as_slice(), &plaintext[..]);

    // Framing/AAD failures remain before private scalar/equation import.
    assert!(matches!(
        without_allocations(|| open_das_ren_private_share(
            &parameters,
            &dealer,
            0,
            recipient.secret(),
            &kem,
            &ciphertext[..123],
            aad
        )),
        Err(ThresholdBlsError::PrivateShareDecryption)
    ));
    assert!(matches!(
        without_allocations(|| open_das_ren_private_share(
            &parameters,
            &dealer,
            0,
            recipient.secret(),
            &kem,
            &ciphertext,
            b""
        )),
        Err(ThresholdBlsError::PrivateShareDecryption)
    ));
    assert!(matches!(
        without_allocations(|| open_das_ren_private_share(
            &parameters,
            &dealer,
            n - 1,
            recipient.secret(),
            &kem,
            &ciphertext,
            aad
        )),
        Err(ThresholdBlsError::InvalidPrivateShare)
    ));
    let mut tampered = ciphertext;
    tampered[123] ^= 1;
    assert!(matches!(
        without_allocations(|| open_das_ren_private_share(
            &parameters,
            &dealer,
            n,
            recipient.secret(),
            &kem,
            &tampered,
            aad
        )),
        Err(ThresholdBlsError::PrivateShareDecryption)
    ));
    // Retrying the unchanged public edge still imports the exact original share.
    let retried = without_allocations(|| {
        open_das_ren_private_share(
            &parameters,
            &dealer,
            n,
            recipient.secret(),
            &kem,
            &ciphertext,
            aad,
        )
        .unwrap()
    });
    assert_eq!(*retried.components_for_authenticated_encryption(), *parts);
}

#[test]
fn fixed_private_edges_preserve_ciphertext_equations_and_no_allocation_at_both_bounds() {
    for n in [4, THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1] {
        check::<BeaconPurpose>(n);
        check::<TleReleasePurpose>(n);
    }
}
