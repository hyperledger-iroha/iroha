//! Real tiny encryption/publication manifests exercise the shared authority checks.
use super::*;

// This constructs the existing tiny-profile crypto/storage fixture, not an
// admitted release source or a 43-record Phase23 correspondence capability.
fn actual_tiny_manifest_v1() -> (
    BgvProfile,
    ZkAmsMkheStreamingCollectiveCiphertextV1,
    ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
) {
    let profile = test_profile();
    let (mut key, secret) = test_key(0xb7);
    // Values below both moduli make A's two residues identical. Use an actual
    // coefficient between them, then reconstruct B=-A*s and the canonical key
    // digest so every limb-swap adversary changes genuine published bytes.
    key.public_a =
        RnsPolynomial::from_unsigned(&profile, &[profile.moduli[1] + 1, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
    key.collective_public_b = key
        .public_a
        .mul(&secret.as_rns(&profile).unwrap(), &profile)
        .unwrap()
        .negate(&profile)
        .unwrap();
    key.digest = collective_public_key_digest(&key, &profile).unwrap();
    key.validate(&profile).unwrap();
    assert_ne!(
        key.public_a.limb(&profile, 0),
        key.public_a.limb(&profile, 1)
    );
    assert_ne!(
        key.collective_public_b.limb(&profile, 0),
        key.collective_public_b.limb(&profile, 1)
    );
    let canonical = test_canonical_plaintext(&[0, 1, 2, 3, 5, 8, 13, 16]);
    let topology = test_input_topology(&profile, b"owned-manifest-preflight");
    let mut key_store = TestStreamingCasV1::new(0x51);
    let authority = test_streaming_key_authority_v1(&profile, &key, &mut key_store, 0);
    let prepared =
        PreparedStreamingCollectiveEncryptionV1::new_v1(&authority.binding, &profile).unwrap();
    let authenticated = prepared.authenticate_key_source_v1(&mut key_store).unwrap();
    let mut active = authenticated
        .activate_v1(
            &authority.binding,
            &canonical,
            topology,
            0,
            &mut KatRandom::new(b"owned-manifest-preflight"),
        )
        .unwrap();
    let mut ciphertext_store = TestStreamingCasV1::new(0x61);
    active
        .publish_all_v1(&mut key_store, &mut ciphertext_store)
        .unwrap();
    let completed = active.finish().unwrap();
    let manifest = ZkAmsMkheStreamingCollectiveCiphertextV1::from_completed_v1(
        completed,
        &authority.binding,
        authority.authority_digest,
        &profile,
    )
    .unwrap();
    manifest.validate_for_profile_v1(&profile).unwrap();
    manifest.validate_key_authority_axes_v1(&authority).unwrap();
    assert_ne!(
        manifest.public_a_limb_pointers[0],
        manifest.public_a_limb_pointers[1]
    );
    assert_ne!(
        manifest.public_b_limb_pointers[0],
        manifest.public_b_limb_pointers[1]
    );
    assert_ne!(
        manifest.public_a_prepass_receipts[0].receipt_digest(),
        manifest.public_a_prepass_receipts[1].receipt_digest()
    );
    assert_ne!(
        manifest.public_b_second_pass_receipts[0].receipt_digest(),
        manifest.public_b_second_pass_receipts[1].receipt_digest()
    );
    (profile, manifest, authority)
}

#[test]
fn actual_manifest_rejects_every_foreign_key_axis_in_the_shared_production_comparison() {
    let (profile, manifest, mut authority) = actual_tiny_manifest_v1();
    for axis in 0..11 {
        let mutate =
            |authority: &mut ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1| match axis {
                0 => authority.binding.profile_digest[0] ^= 1,
                1 => authority.binding.security_certificate_digest[0] ^= 1,
                2 => authority.binding.roster_digest[0] ^= 1,
                3 => authority.binding.key_material_digest[0] ^= 1,
                4 => authority.binding.epoch ^= 1,
                5 => authority.binding.transcript_digest[0] ^= 1,
                6 => authority.binding.key_digest[0] ^= 1,
                7 => authority.binding.binding_digest[0] ^= 1,
                8 => authority.authority_digest[0] ^= 1,
                9 => authority.binding.public_a_limb_pointers.swap(0, 1),
                10 => authority.binding.public_b_limb_pointers.swap(0, 1),
                _ => unreachable!(),
            };
        mutate(&mut authority);
        // The genuine manifest remains internally valid. Its exact retained
        // key relationship, not merely its self-hash, must reject the splice.
        manifest.validate_for_profile_v1(&profile).unwrap();
        assert_eq!(
            manifest.validate_key_authority_axes_v1(&authority),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext),
            "foreign key axis {axis}",
        );
        mutate(&mut authority);
        manifest.validate_key_authority_axes_v1(&authority).unwrap();
    }
}

#[test]
fn actual_manifest_rejects_resealed_pointer_receipt_and_component_order_mutants() {
    let (profile, mut manifest, authority) = actual_tiny_manifest_v1();
    for axis in 0..8 {
        let mutate = |manifest: &mut ZkAmsMkheStreamingCollectiveCiphertextV1| match axis {
            0 => manifest.public_a_prepass_receipts.swap(0, 1),
            1 => manifest.public_b_second_pass_receipts.swap(0, 1),
            2 => manifest.constant_limb_pointers.swap(0, 1),
            3 => manifest.linear_limb_pointers.swap(0, 1),
            4 => manifest.constant_publication_receipts.swap(0, 1),
            5 => manifest.linear_publication_receipts.swap(0, 1),
            6 => core::mem::swap(
                &mut manifest.constant_limb_pointers,
                &mut manifest.linear_limb_pointers,
            ),
            7 => core::mem::swap(
                &mut manifest.public_a_prepass_receipts,
                &mut manifest.public_b_prepass_receipts,
            ),
            _ => unreachable!(),
        };
        let original_digest = manifest.manifest_digest;
        mutate(&mut manifest);
        // Recompute the metadata digest so rejection must reach actual typed
        // pointer/receipt/second-read and exact component-order validation.
        manifest.manifest_digest =
            streaming_collective_ciphertext_manifest_digest_v1(&manifest, &profile).unwrap();
        assert_ne!(
            manifest.manifest_digest, original_digest,
            "nontrivial mutation {axis}"
        );
        let expected = match axis {
            0 | 1 | 6 | 7 => ZkAmsMkheErrorV1::InvalidKeyMaterial,
            2..=5 => ZkAmsMkheErrorV1::InvalidCiphertext,
            _ => unreachable!(),
        };
        assert_eq!(
            manifest.validate_for_profile_v1(&profile),
            Err(expected),
            "resealed receipt/order axis {axis}",
        );
        assert!(manifest.sealed_binding_with_profile_v1(&profile).is_err());
        mutate(&mut manifest);
        manifest.manifest_digest =
            streaming_collective_ciphertext_manifest_digest_v1(&manifest, &profile).unwrap();
        manifest.validate_for_profile_v1(&profile).unwrap();
        manifest.validate_key_authority_axes_v1(&authority).unwrap();
    }
}
