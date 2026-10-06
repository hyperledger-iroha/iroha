//! Bounded original frames, fixed native identities and complete catalog regressions.

use super::*;

#[test]
#[ignore = "genuine complete k12/k16 native verifier inventory; run optimized"]
fn genuine_signed_complete_inventory_mounts_and_rejects_substituted_originals() {
    let (pack, installation) = engineering_fixture::signed_inventory();
    let original = pack.to_canonical_bytes().unwrap();
    let installed = InstalledVerifierPackV1::load(&original, installation).unwrap();
    assert_eq!(installed.original(), original);
    assert_eq!(installed.originals(), &pack);
    assert_eq!(
        installed.verifier().scheme().scheme_id(),
        installation.scheme_id
    );
    for pin in [[0; 32], [0x52; 32]] {
        assert!(
            InstalledVerifierPackV1::load(
                &original,
                InstallationV1 {
                    manifest_digest: pin,
                    ..installation
                }
            )
            .is_err()
        );
    }
    for changed_member in 0..7 {
        let mut changed = pack.clone();
        let member = match changed_member {
            0 => &mut changed.scheme,
            1 => &mut changed.signer_certificate,
            2 => &mut changed.manifest,
            3 => &mut changed.steps[0].artifact.descriptor,
            4 => &mut changed.steps[0].artifact.verifying_key,
            5 => &mut changed.lineage.descriptor,
            _ => &mut changed.lineage.verifying_key,
        };
        *member.last_mut().unwrap() ^= 1;
        assert!(
            InstalledVerifierPackV1::load(&changed.to_canonical_bytes().unwrap(), installation)
                .is_err(),
            "substituted authority/native original {changed_member}"
        );
    }
}

fn kind(tag: u8) -> KagemushaWalletOperationKindV1 {
    *KagemushaWalletOperationKindV1::ALL
        .iter()
        .find(|kind| kind.tag() == tag)
        .unwrap()
}

fn structural_pack() -> VerifierPackV1 {
    // Deliberately invalid authority/proof bytes. Only bounded canonical carrier tests
    // use this fixture; it cannot construct an InstalledVerifierPackV1 or native verdict.
    VerifierPackV1 {
        version: 1,
        scheme: vec![1],
        signer_certificate: vec![2],
        manifest: vec![3],
        allowlist: vec![4],
        steps: SIGMA_CATALOG_V1
            .iter()
            .map(|(tag, mask)| StepOriginalV1 {
                kind: kind(*tag),
                enabled_controls: *mask,
                artifact: ArtifactOriginalV1 {
                    descriptor: vec![*tag],
                    verifying_key: vec![*mask as u8 + 1],
                },
            })
            .collect(),
        lineage: ArtifactOriginalV1 {
            descriptor: vec![17],
            verifying_key: vec![18],
        },
    }
}

#[test]
fn canonical_carrier_retains_every_original_and_rejects_trailing_bytes() {
    let pack = structural_pack();
    let original = pack.to_canonical_bytes().unwrap();
    assert_eq!(VerifierPackV1::decode_canonical(&original).unwrap(), pack);
    let mut trailing = original.clone();
    trailing.push(0);
    assert!(VerifierPackV1::decode_canonical(&trailing).is_err());
    let mut changed_version = pack.clone();
    changed_version.version = 2;
    assert!(changed_version.to_canonical_bytes().is_err());
    assert_eq!(
        InstalledVerifierPackV1::load(
            &original,
            InstallationV1 {
                scheme_id: [1; 32],
                manifest_digest: [2; 32],
            }
        )
        .err(),
        Some(Error::Authority)
    );
    assert_eq!(pack.runtime_bindings(), Err(Error::Inventory));
}

#[test]
fn full_catalog_rejects_missing_extra_reordered_and_undefined_selectors() {
    let pack = structural_pack();
    for missing in 0..SIGMA_CATALOG_V1.len() {
        let mut changed = pack.clone();
        changed.steps.remove(missing);
        assert_eq!(changed.inventory_bounds(), Err(Error::Inventory));
    }
    let mut extra = pack.clone();
    extra.steps.push(extra.steps[0].clone());
    assert_eq!(extra.inventory_bounds(), Err(Error::Inventory));
    for index in 0..SIGMA_CATALOG_V1.len() - 1 {
        let mut changed = pack.clone();
        changed.steps.swap(index, index + 1);
        assert_eq!(changed.inventory_bounds(), Err(Error::Inventory));
    }
    let mut mask = pack.clone();
    mask.steps[0].enabled_controls = 1;
    assert_eq!(mask.inventory_bounds(), Err(Error::Inventory));
    let mut receive = pack.clone();
    receive.steps[11].enabled_controls = 2;
    assert_eq!(receive.inventory_bounds(), Err(Error::Inventory));
    assert!(
        KagemushaWalletOperationKindV1::ALL
            .iter()
            .all(|kind| SIGMA_CATALOG_V1.contains(&(kind.tag(), 0)))
    );
    assert_eq!(
        SIGMA_CATALOG_V1.iter().filter(|(tag, _)| *tag == 3).count(),
        8
    );
}

#[test]
fn all_resource_caps_apply_before_profile_or_authority_decode() {
    let mut total = 0;
    let empty = ArtifactOriginalV1 {
        descriptor: vec![],
        verifying_key: vec![1],
    };
    assert_eq!(artifact_bounds(&empty, &mut total), Err(Error::Inventory));
    let oversized = ArtifactOriginalV1 {
        descriptor: vec![0; DESCRIPTOR_MAX_BYTES_V1 + 1],
        verifying_key: vec![1],
    };
    assert_eq!(
        artifact_bounds(&oversized, &mut total),
        Err(Error::Inventory)
    );
    let oversized_key = ArtifactOriginalV1 {
        descriptor: vec![1],
        verifying_key: vec![0; VERIFYING_KEY_MAX_BYTES_V1 + 1],
    };
    assert_eq!(
        artifact_bounds(&oversized_key, &mut total),
        Err(Error::Inventory)
    );
    total = INVENTORY_MAX_BYTES_V1 - 1;
    assert_eq!(
        artifact_bounds(
            &ArtifactOriginalV1 {
                descriptor: vec![1],
                verifying_key: vec![1]
            },
            &mut total
        ),
        Err(Error::Inventory)
    );
    total = usize::MAX;
    assert_eq!(
        artifact_bounds(
            &ArtifactOriginalV1 {
                descriptor: vec![1],
                verifying_key: vec![1]
            },
            &mut total
        ),
        Err(Error::Inventory)
    );
    assert!(VerifierPackV1::decode_canonical(&vec![0; VERIFIER_PACK_MAX_BYTES_V1 + 1]).is_err());
    assert!(VerifierPackV1::decode_canonical(&[]).is_err());
    let mut pack = structural_pack();
    pack.manifest
        .resize(KAGEMUSHA_WALLET_ARTIFACT_MANIFEST_MAX_BYTES_V1 + 1, 0);
    assert_eq!(pack.bounds(), Err(Error::Inventory));
}

#[test]
fn independent_installation_is_nonzero_and_never_selected_from_pack() {
    let original = structural_pack().to_canonical_bytes().unwrap();
    for installation in [
        InstallationV1 {
            scheme_id: [0; 32],
            manifest_digest: [1; 32],
        },
        InstallationV1 {
            scheme_id: [1; 32],
            manifest_digest: [0; 32],
        },
    ] {
        assert_eq!(
            InstalledVerifierPackV1::load(&original, installation).err(),
            Some(Error::RuntimeBinding)
        );
    }
}

#[test]
fn native_protocol_preimages_bind_actual_curve_tables_and_params() {
    let eq = eq_protocol_transcript_v1().unwrap();
    let ep = ep_protocol_transcript_v1().unwrap();
    assert_ne!(eq, ep);
    assert_eq!(&eq[..5], &[1, 0, 1, 0, 1]);
    assert_eq!(&ep[..5], &[1, 0, 1, 0, 0]);
    assert_eq!(&eq[5..37], &CurveV1::Vesta.base_modulus());
    assert_eq!(&eq[37..69], &CurveV1::Vesta.scalar_modulus());
    for (curve, bytes) in [(CurveV1::Vesta, eq), (CurveV1::Pallas, ep)] {
        for k in 12..=16 {
            let params = pinned_params_digest(curve, k).unwrap();
            assert!(bytes.windows(params.len()).any(|window| window == params));
        }
        assert!(bytes.windows(8).any(|window| window == Domain::Proof.tag()));
        assert!(bytes.windows(8).any(|window| window == Domain::Fold.tag()));
    }
}

#[test]
fn native_profile_contains_fixed_typed_policies_and_all_decision_codes() {
    let sigma = policy(false);
    let omega = policy(true);
    assert_eq!(sigma.curve, CurveV1::Vesta);
    assert_eq!(sigma.instance_lengths, vec![1]);
    assert_eq!(sigma.instance_types, vec![InstanceType::Bounded]);
    assert_eq!(omega.curve, CurveV1::Pallas);
    assert_eq!((omega.min_k, omega.max_k), (16, 16));
    assert_eq!(omega.instance_lengths, vec![1, 2, 16]);
    assert_eq!(
        omega.instance_types,
        vec![
            InstanceType::Bounded,
            InstanceType::Field,
            InstanceType::Bounded
        ]
    );
    let profile = native_profile_transcript_v1().unwrap();
    assert_eq!(&profile[..3], &[1, 0, 1]);
    assert!(profile.ends_with(b"kgwomg_1\x01\x02\x03\x04"));
    let (_, counts) = profile.split_at(profile.len() - 12 - 32);
    let expected: Vec<_> = [33_u32, 8, 26, 18, 52, 16, 544, 1088]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(&counts[..32], expected);
}

#[test]
fn inventory_item_transcript_binds_raw_bytes_shape_and_full_digests() {
    let mut item = ParsedArtifact {
        curve: CurveV1::Vesta,
        k: 12,
        descriptor_bytes: 11,
        descriptor_sha256: [2; 32],
        verifying_key_bytes: 13,
        verifying_key_sha256: [3; 32],
        descriptor_digest: [4; 32],
        verifying_key_digest: [5; 32],
        proof_bytes: 17,
    };
    let mut original = Vec::new();
    item.append(&mut original).unwrap();
    assert_eq!(original.len(), 142);
    assert_eq!(&original[..6], &[1, 12, 11, 0, 0, 0]);
    let digest = artifact_digest(b"artifact-inventory", &original);
    item.verifying_key_sha256[0] ^= 1;
    let mut changed = Vec::new();
    item.append(&mut changed).unwrap();
    assert_ne!(digest, artifact_digest(b"artifact-inventory", &changed));
    assert_ne!(digest, artifact_digest(b"native-profile", &original));
    assert_ne!(
        digest,
        artifact_digest(b"artifact-inventory", &original[..original.len() - 1])
    );
    let mut independently_framed = Vec::new();
    independently_framed.extend_from_slice(KAGEMUSHA_WALLET_DIGEST_PREFIX_V1);
    independently_framed.extend_from_slice(b"artifact-inventory\0");
    independently_framed.extend_from_slice(&(original.len() as u64).to_le_bytes());
    independently_framed.extend_from_slice(&original);
    assert_eq!(
        digest,
        <[u8; 32]>::from(Sha256::digest(&independently_framed))
    );
}

#[test]
fn frozen_protocol_digest_matches_independently_framed_native_originals() {
    // Golden preimages independently constructed from native source moduli, the checked-in
    // RP57 table originals and PINNED_PARAMS_V1. No test-produced key or verdict is used.
    let body = eq_protocol_transcript_v1().unwrap();
    assert_eq!(body.len(), 423);
    assert_eq!(
        hex::encode(Sha256::digest(&body)),
        "16793a98e248683d849e1c29b1022b408edb14af2f6f15bc95c7a61f2f79f4ea"
    );
    assert_eq!(
        hex::encode(artifact_digest(b"eq-protocol", &body)),
        "f8740dd567145d4146d8a970339bc72205ba4093aacb2c451702ce741b58c273"
    );
    let body = ep_protocol_transcript_v1().unwrap();
    assert_eq!(body.len(), 423);
    assert_eq!(
        hex::encode(Sha256::digest(&body)),
        "16fefdc12b8fa67594969413bec611825ca1db942b4782b4d6bc0eca06428d6d"
    );
    assert_eq!(
        hex::encode(artifact_digest(b"ep-protocol", &body)),
        "d828551f4e97743077174b94408253115fb622fdb0132ede8ed7d722de09fc3c"
    );
}

#[test]
fn native_verifier_profile_matches_the_full_native_encoder_golden() {
    // Captured from the actual Native encoder; this freezes verifier family1 only.
    // Complete producer schedules and proving artifacts require their own final profile.
    let original = hex::decode(concat!(
        "010001100000000100000000020000000003000000000301000000030200000003030000000304000000030500000003",
        "0600000003070000000400000000040100000005000000000600000000070000000008000000005f0000004e52543000",
        "001c14315baa201047a077821817683104003700000000000000587ed1b1434089fd020201000401000000010c011004",
        "02000000040100000004010000000d010000000000000004010000000d01000000000000000401000000730000004e52",
        "543000001c14315baa201047a077821817683104004b000000000000000dea5f0df63cb40b0202010004000000000110",
        "011004020000000401000000040100000017030000000000000004010000000402000000041000000017030000000000",
        "000004010000000400000000040100000021000000080000001a00000012000000340000001000000020020000400400",
        "006b67776f6d675f3101020304",
    ))
    .unwrap();
    let body = native_profile_transcript_v1().unwrap();
    assert_eq!(body, original);
    assert_eq!(body.len(), 349);
    assert_eq!(
        hex::encode(Sha256::digest(&body)),
        "0f2385154950ff4e227a7bb5cac9aea258411b855f4c46913f88d8e13c01f6ca"
    );
    assert_eq!(
        hex::encode(artifact_digest(b"native-profile", &body)),
        "97f358afb29091461322d243c4e2fa01e1a9d8c7b47f202e7fc00c5062f32b26"
    );
}
