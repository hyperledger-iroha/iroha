//! Physical allocation and exact transcript controls for fixed hybrid owners.
use super::*;
use crate::test_allocations::without_allocations;
use hkdf::Hkdf;
use rand_core::OsRng;

#[test]
fn fixed_hybrid_generation_clone_exchange_and_import_do_not_allocate() {
    // Exercise platform RNG initialization before measuring the actual production
    // operations. The measured calls still obtain genuine OS entropy.
    let warm = HybridKeyPair::try_generate(&mut OsRng).unwrap();
    encapsulate(
        HybridSuite::X25519MlKem768ChaCha20Poly1305,
        warm.public(),
        &mut OsRng,
    )
    .unwrap();
    let pair = without_allocations(|| HybridKeyPair::try_generate(&mut OsRng).unwrap());
    let cloned = without_allocations(|| pair.clone());
    assert!(
        core::ptr::eq(pair.public(), pair.secret().public()),
        "keypair has one public owner"
    );
    let exported = without_allocations(|| pair.secret().to_bytes());
    let imported =
        without_allocations(|| HybridSecretKey::from_bytes(exported.0, exported.1).unwrap());
    assert_eq!(imported.kyber_bytes(), pair.secret().kyber_bytes());
    assert_eq!(cloned.public().kyber_bytes(), pair.public().kyber_bytes());
    let public = without_allocations(|| {
        HybridPublicKey::from_bytes(pair.public().x25519_bytes(), pair.public().kyber_bytes())
            .unwrap()
    });
    let (ciphertext, sealed) = without_allocations(|| {
        encapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            &public,
            &mut OsRng,
        )
        .unwrap()
    });
    let copied = without_allocations(|| ciphertext.clone());
    let imported_ciphertext = without_allocations(|| {
        HybridKemCiphertext::from_parts(copied.ephemeral_public(), copied.kyber_ciphertext())
            .unwrap()
    });
    assert_eq!(ciphertext, imported_ciphertext);
    let opened = without_allocations(|| {
        decapsulate(
            HybridSuite::X25519MlKem768ChaCha20Poly1305,
            &imported_ciphertext,
            &imported,
        )
        .unwrap()
    });
    assert_eq!(sealed.encryption_key(), opened.encryption_key());
    assert_eq!(sealed.rekey_secret(), opened.rekey_secret());
    assert_eq!(
        core::mem::size_of::<HybridPublicKey>(),
        32 + HYBRID_KEM_SUITE.public_key_len()
    );
    assert_eq!(
        core::mem::size_of::<HybridKemCiphertext>(),
        32 + HYBRID_KEM_SUITE.ciphertext_len()
    );
    assert_eq!(
        core::mem::size_of::<HybridKeyPair>(),
        core::mem::size_of::<HybridSecretKey>()
    );
}

#[test]
fn streaming_hkdf_matches_original_length_prefixed_transcript_exactly() {
    let suite = HybridSuite::X25519MlKem768ChaCha20Poly1305;
    let ecdh = [0x21; 32];
    let kem_shared = [0x42; 32];
    let recipient = [0x17; 32];
    let ephemeral = [0x35; 32];
    let public = [0x73; HYBRID_KEM_SUITE.public_key_len()];
    let ciphertext = [0x64; HYBRID_KEM_SUITE.ciphertext_len()];
    for (public, ciphertext) in [
        (&public[..], &ciphertext[..]),
        (&[][..], &[][..]),
        (&public[..17], &ciphertext[..19]),
    ] {
        let transcript = HybridTranscript {
            recipient_x25519: &recipient,
            recipient_kyber: public,
            ephemeral_x25519: &ephemeral,
            kyber_ciphertext: ciphertext,
        };
        let actual =
            without_allocations(|| derive_material(suite, &ecdh, &kem_shared, transcript).unwrap());
        // Independent old concatenation, only in the test and outside observation.
        let mut original = Zeroizing::new(Vec::new());
        original.extend_from_slice(&ecdh);
        original.extend_from_slice(&kem_shared);
        for part in [
            SUITE_TRANSCRIPT_DOMAIN_V1,
            &recipient,
            public,
            &ephemeral,
            ciphertext,
        ] {
            original.extend_from_slice(&u64::try_from(part.len()).unwrap().to_be_bytes());
            original.extend_from_slice(part);
        }
        let reference = Hkdf::<Sha3_256>::new(Some(suite.hkdf_salt()), &original);
        let mut material = Zeroizing::new([0; 64]);
        let mut rekey = Zeroizing::new([0; 32]);
        reference
            .expand(suite.hkdf_info(), &mut material[..])
            .unwrap();
        reference
            .expand(suite.rekey_info(), &mut rekey[..])
            .unwrap();
        assert_eq!(actual.encryption_key(), material[..32]);
        assert_eq!(actual.rekey_secret(), *rekey);
    }
}

#[test]
fn initialized_mlkem_kernels_have_no_output_backing_allocation() {
    use soranet_pq::deterministic_chacha20_rng;
    let mut rng =
        deterministic_chacha20_rng(HedgedRngSeed::from_entropy([0x65; 32]), b"fixed-outputs");
    let mut public = [0; HYBRID_KEM_SUITE.public_key_len()];
    let mut secret = Zeroizing::new([0; HYBRID_KEM_SUITE.secret_key_len()]);
    let mut shared = Zeroizing::new([0; HYBRID_KEM_SUITE.shared_secret_len()]);
    let mut ciphertext = [0; HYBRID_KEM_SUITE.ciphertext_len()];
    let mut opened = Zeroizing::new([0; HYBRID_KEM_SUITE.shared_secret_len()]);
    without_allocations(|| {
        generate_mlkem_keypair_into(HYBRID_KEM_SUITE, &mut rng, &mut public, &mut secret[..])
            .unwrap()
    });
    without_allocations(|| {
        encapsulate_mlkem_into(
            HYBRID_KEM_SUITE,
            &public,
            &mut rng,
            &mut shared[..],
            &mut ciphertext,
        )
        .unwrap()
    });
    without_allocations(|| {
        decapsulate_mlkem_into(HYBRID_KEM_SUITE, &secret[..], &ciphertext, &mut opened[..]).unwrap()
    });
    assert_eq!(*shared, *opened);
}
