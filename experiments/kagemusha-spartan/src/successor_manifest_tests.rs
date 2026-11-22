use super::*;
use crate::native::program_bundle::tests::original;
use iroha_pasta::{Fp, Fq};

#[test]
fn artifact_signed_five_kind_manifest_requires_exact_genesis_programs_and_profile() {
    use crate::native::{ProgramProfileV2, device_fixture, engine};
    use iroha_data_model::sumeragi_finality::{
        SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture,
    };
    let p = std::array::from_fn::<_, 5, _>(|i| original::<Fp>(true, i, 55, 13, true));
    let s = std::array::from_fn::<_, 5, _>(|i| original::<Fq>(false, i, 54, 10, false));
    let profile = ProgramProfileV2 {
        primary: p.each_ref().map(|o| o.1),
        secondary: s.each_ref().map(|o| o.1),
        dimensions: [[24, 25], [24, 24]],
    };
    let mut tapes = ProgramBundleV2 {
        version: 2,
        primary: p.map(|o| o.0),
        secondary: s.map(|o| o.0),
    };
    let bundle = tapes.encode().unwrap();
    let scheme = device_fixture::scheme();
    let fixture = NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
    let finality = SumeragiCommitVerifierV1::new(&fixture.verifier()).unwrap();
    let cert = engine::test_artifact_certificate(&scheme.scheme_id());
    let artifact = [cert.body.as_slice(), cert.signature.as_slice()].concat();
    let (manifest, selected) = profile
        .prepare_manifest_for_signing(&bundle, &scheme, &finality, &artifact)
        .unwrap();
    let signature = engine::test_sign_manifest(&manifest);
    let geometry = profile
        .verify_signed_originals(
            &bundle, &manifest, &signature, &selected, &scheme, &finality,
        )
        .unwrap();
    assert_eq!(geometry.map(|g| g.column_bits), [13, 10]);
    // Both wire dimensions are distinct from the installed eight-tape generation.
    assert!(crate::native::decode::<crate::native::ParameterManifest>(&manifest).is_err());
    assert!(crate::native::ProgramBundle::decode(&bundle).is_err());
    let wrong = NativeFinalityFixture::start_with_explicit_parameters("separate-chain");
    let other_finality = SumeragiCommitVerifierV1::new(&wrong.verifier()).unwrap();
    assert!(
        profile
            .verify_signed_originals(
                &bundle,
                &manifest,
                &signature,
                &selected,
                &scheme,
                &other_finality
            )
            .is_err()
    );
    let enrollment = device_fixture::enrollment_certificate();
    let wrong_role = [enrollment.body.as_slice(), enrollment.signature.as_slice()].concat();
    assert!(
        profile
            .prepare_manifest_for_signing(&bundle, &scheme, &finality, &wrong_role)
            .is_err()
    );
    let mut altered_signature = signature;
    altered_signature[63] ^= 1;
    assert!(
        profile
            .verify_signed_originals(
                &bundle,
                &manifest,
                &altered_signature,
                &selected,
                &scheme,
                &finality
            )
            .is_err()
    );
    let mut changed_pin = profile.clone();
    changed_pin.secondary.swap(3, 4);
    assert!(
        changed_pin
            .verify_signed_originals(
                &bundle, &manifest, &signature, &selected, &scheme, &finality
            )
            .is_err()
    );
    assert!(
        profile
            .verify_signed_originals(&bundle, &manifest, &signature, &[0; 32], &scheme, &finality)
            .is_err()
    );
    tapes.primary.swap(3, 4);
    assert!(
        profile
            .verify_signed_originals(
                &tapes.encode().unwrap(),
                &manifest,
                &signature,
                &selected,
                &scheme,
                &finality
            )
            .is_err()
    );
    // A real Artifact signature cannot authorize different generator semantics.
    let mut wrong_manifest: ParameterManifestV5 = crate::native::decode(&manifest).unwrap();
    wrong_manifest.table_construction.push('!');
    let changed = crate::native::encode(&wrong_manifest).unwrap();
    let wrong_signature = engine::test_sign_manifest(&changed);
    let wrong_digest = engine::manifest_signing_message(&changed).to_repr();
    assert!(matches!(
        profile.verify_signed_originals(
            &bundle,
            &changed,
            &wrong_signature,
            &wrong_digest,
            &scheme,
            &finality
        ),
        Err(Error::AuthorityMismatch(
            "canonical five-kind generator and table profile"
        ))
    ));
    if let Some(dir) = std::env::var_os("KAGEMUSHA_SPARTAN_MANIFEST_FIXTURE_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir(&dir).unwrap();
        let genesis_key = fixture
            .genesis()
            .external_transactions()
            .next()
            .unwrap()
            .authority()
            .try_signatory()
            .unwrap()
            .to_string();
        for (name, bytes) in [
            ("scheme.norito", scheme.to_canonical_bytes().unwrap()),
            ("genesis.norito", fixture.genesis().encode_wire().unwrap()),
            ("certificate.bin", artifact),
            ("enrollment.bin", wrong_role),
            ("manifest.norito", manifest),
            ("signature.bin", signature.to_vec()),
            ("wrong-manifest.norito", changed),
            ("wrong-signature.bin", wrong_signature.to_vec()),
            (
                "identity.bin",
                [scheme.scheme_id().as_slice(), &selected, &wrong_digest].concat(),
            ),
            ("genesis-key", genesis_key.into_bytes()),
        ] {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }
}

#[test]
fn five_kind_issuance_obeys_the_complete_governed_frame_limit() {
    use iroha_data_model::isi::kagemusha_wallet::spartan::InstallKagemushaSpartanProgramsV1 as Install;
    let manifest = vec![2; Install::MAX_MANIFEST_BYTES];
    let _fixed = norito::core::WireLayoutGuard::enter(norito::core::WireLayout::Fixed);
    let mut row = Install {
        scheme: [1; 32],
        asset: [2; 32],
        manifest_digest: [3; 32],
        manifest: manifest.clone(),
        signature: [4; 64],
        programs: vec![5; Install::MAX_BYTES],
    };
    let overhead = norito::core::encoded_frame_len(&row).unwrap() - row.programs.len();
    row.programs.truncate(Install::MAX_BYTES - overhead);
    assert_eq!(
        norito::core::encoded_frame_len(&row).unwrap(),
        Install::MAX_BYTES
    );
    require_install_extent(&row.programs, &manifest).unwrap();
    row.programs.push(0);
    assert!(matches!(
        require_install_extent(&row.programs, &manifest),
        Err(Error::ResourceLimit)
    ));
    assert!(matches!(
        require_install_extent(&[7], &vec![0; Install::MAX_MANIFEST_BYTES + 1]),
        Err(Error::ResourceLimit)
    ));
}
