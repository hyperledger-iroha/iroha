//! Allocation-free borrowed validation preserves owning constructors and error ordering.

use super::*;
use crate::test_allocations::without_allocations;
use rand::SeedableRng as _;
use rand_chacha::ChaCha20Rng;

fn public(x25519: &[u8], kyber: &[u8], expected: Result<(), HybridError>) {
    assert_eq!(
        without_allocations(|| HybridPublicKey::validate_bytes(x25519, kyber)),
        expected
    );
    let owned = HybridPublicKey::from_bytes(x25519, kyber);
    assert_eq!(owned.as_ref().map(|_| ()).map_err(Clone::clone), expected);
    if let Ok(owned) = owned {
        assert_eq!(owned.x25519_bytes().as_slice(), x25519);
        assert_eq!(owned.kyber_bytes(), kyber);
    }
}

fn ciphertext(x25519: &[u8], kyber: &[u8], expected: Result<(), HybridError>) {
    assert_eq!(
        without_allocations(|| HybridKemCiphertext::validate_parts(x25519, kyber)),
        expected
    );
    let owned = HybridKemCiphertext::from_parts(x25519, kyber);
    assert_eq!(owned.as_ref().map(|_| ()).map_err(Clone::clone), expected);
    if let Ok(owned) = owned {
        assert_eq!(owned.ephemeral_public().as_slice(), x25519);
        assert_eq!(owned.kyber_ciphertext(), kyber);
    }
}

#[test]
fn borrowed_public_keys_validate_real_material_and_reject_each_component_without_allocating() {
    for seed in [9, 84] {
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        let pair = HybridKeyPair::generate(&mut rng).unwrap();
        let x25519 = pair.public().x25519_bytes();
        let kyber = pair.public().kyber_bytes();
        public(&x25519, kyber, Ok(()));
        for length in [0, 31, 33] {
            public(
                &vec![0; length],
                &[],
                Err(HybridError::InvalidX25519PublicKeyLength {
                    expected: 32,
                    found: length,
                }),
            );
        }
        let mut one = [0; 32];
        one[0] = 1;
        for low_order in [[0; 32], one] {
            public(&low_order, &[], Err(HybridError::InvalidX25519PublicKey));
        }
        for length in [0, kyber.len() - 1, kyber.len() + 1] {
            public(
                &x25519,
                &vec![0; length],
                Err(HybridError::InvalidKyberPublicKeyLength {
                    expected: kyber.len(),
                    found: length,
                }),
            );
        }
        public(
            &x25519,
            &vec![0; kyber.len()],
            Err(HybridError::InvalidKyberPublicKey),
        );
        // Exercise both packed coefficients at the front and end of the polynomial.
        for index in [0, 1, 766, 767] {
            for coefficient in [3328_u16, 3329, 4095] {
                let mut changed = kyber.to_vec();
                let offset = (index / 2) * 3;
                if index % 2 == 0 {
                    changed[offset] = coefficient as u8;
                    changed[offset + 1] = (changed[offset + 1] & 0xf0) | ((coefficient >> 8) as u8);
                } else {
                    changed[offset + 1] = (changed[offset + 1] & 0x0f) | ((coefficient << 4) as u8);
                    changed[offset + 2] = (coefficient >> 4) as u8;
                }
                public(
                    &x25519,
                    &changed,
                    if coefficient < 3329 {
                        Ok(())
                    } else {
                        Err(HybridError::InvalidKyberPublicKey)
                    },
                );
            }
        }
    }
}

#[test]
fn borrowed_ciphertexts_preserve_exact_encoding_checks_without_allocating_or_decrypting() {
    let mut rng = ChaCha20Rng::from_seed([91; 32]);
    let pair = HybridKeyPair::generate(&mut rng).unwrap();
    let (encrypted, _) = encapsulate(
        HybridSuite::X25519MlKem768ChaCha20Poly1305,
        pair.public(),
        &mut rng,
    )
    .unwrap();
    let x25519 = encrypted.ephemeral_public();
    let kyber = encrypted.kyber_ciphertext();
    ciphertext(x25519, kyber, Ok(()));
    for length in [0, 31, 33] {
        ciphertext(
            &vec![0; length],
            &[],
            Err(HybridError::InvalidX25519PublicKeyLength {
                expected: 32,
                found: length,
            }),
        );
    }
    let mut one = [0; 32];
    one[0] = 1;
    for low_order in [[0; 32], one] {
        ciphertext(&low_order, &[], Err(HybridError::InvalidX25519PublicKey));
    }
    for length in [0, kyber.len() - 1, kyber.len(), kyber.len() + 1] {
        ciphertext(
            x25519,
            &vec![0; length],
            Err(HybridError::InvalidKyberCiphertext),
        );
    }
    let mut changed = kyber.to_vec();
    changed[17] ^= 0x80;
    // Encoding validity grants no claim that a ciphertext opens or is authenticated.
    ciphertext(x25519, &changed, Ok(()));
}
