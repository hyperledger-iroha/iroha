//! Desktop matrix and staging preserve the shared original preset selection.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_deploy::bootstrap::InstalledNetworkProfile;

fn image(name: &str) -> Vec<u8> {
    InstalledNetworkProfiles::new(vec![
        InstalledNetworkProfile::new(
            name.into(),
            KeyPair::from_seed(vec![27; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
            7,
            "https://fixture.invalid/checkpoint.nrt".into(),
        )
        .unwrap(),
    ])
    .unwrap()
    .encode_installation()
    .unwrap()
}

#[test]
fn matrix_cannot_claim_complete_qualification_or_lose_original_profile_custody() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("bundle");
    let selection = network_profiles::development(
        &InstalledNetworkProfiles::from_installation_bytes(&image("fixture")).unwrap(),
    )
    .unwrap();
    selection
        .stage(&NativeBundleLayout::current().profiles_path(&bundle))
        .unwrap();
    let manifest = bundle.join("manifest.json");
    std::fs::write(&manifest, b"{}").unwrap();
    let mut result = super::MochiBundleResult {
        target: "fixture".into(),
        profile: "debug".into(),
        bundle_name: "bundle".into(),
        output_root: root.path().into(),
        bundle_root: bundle.clone(),
        manifest_path: manifest,
        archive_path: None,
        archive_sha256: None,
        network_profiles: Some(selection),
    };
    let matrix = root.path().join("matrix.json");
    super::update_bundle_matrix(&result, &matrix, true).unwrap();
    let value: Value = json::from_slice(&std::fs::read(&matrix).unwrap()).unwrap();
    assert_eq!(
        value["entries"][0]["qualification"].as_str(),
        Some("local_native_diagnostic")
    );
    assert_eq!(
        value["entries"][0]["official_taira_attachment_qualified"].as_bool(),
        Some(false)
    );
    assert_eq!(
        value["entries"][0]["network_profiles"],
        result.network_profiles.as_ref().unwrap().provenance()
    );
    let original = std::fs::read(&matrix).unwrap();
    std::fs::remove_file(NativeBundleLayout::current().profiles_path(&bundle)).unwrap();
    assert!(super::update_bundle_matrix(&result, &matrix, true).is_err());
    assert_eq!(std::fs::read(&matrix).unwrap(), original);
    result.network_profiles = None;
    result.profile = "release".into();
    assert!(super::run_bundle_smoke(&result).is_err());
    assert!(super::update_bundle_matrix(&result, &matrix, true).is_err());
    assert_eq!(std::fs::read(&matrix).unwrap(), original);
}

#[test]
fn staging_revalidates_original_profile_and_archive_before_replacing_outputs() {
    use super::{MochiBundleResult, archive_digest, stage_bundle};

    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("bundle");
    let bytes = image("fixture");
    let selection = network_profiles::development(
        &InstalledNetworkProfiles::from_installation_bytes(&bytes).unwrap(),
    )
    .unwrap();
    selection
        .stage(&NativeBundleLayout::current().profiles_path(&bundle))
        .unwrap();
    let manifest = bundle.join("manifest.json");
    std::fs::write(&manifest, b"{}").unwrap();
    // Opaque component bytes test copy/digest custody, not the archive producer or native runtime.
    let archive = root.path().join("bundle.tar.gz");
    std::fs::write(&archive, b"original archive bytes").unwrap();
    let result = MochiBundleResult {
        target: "fixture".into(),
        profile: "debug".into(),
        bundle_name: "bundle".into(),
        output_root: root.path().into(),
        bundle_root: bundle.clone(),
        manifest_path: manifest,
        archive_sha256: Some(archive_digest(&archive).unwrap()),
        archive_path: Some(archive.clone()),
        network_profiles: Some(selection),
    };
    let stage = root.path().join("staged");
    let absent_stage = root.path().join("absent-stage");
    let kept = stage.join("bundle").join("kept");
    std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
    std::fs::write(&kept, b"previous staged bundle").unwrap();
    let profile = NativeBundleLayout::current().profiles_path(&bundle);
    for changed in [Some(image("changed")), None] {
        if let Some(changed) = changed {
            std::fs::write(&profile, changed).unwrap();
        } else {
            std::fs::remove_file(&profile).unwrap();
        }
        assert!(stage_bundle(&result, &stage).is_err());
        assert!(stage_bundle(&result, &absent_stage).is_err());
        assert!(!absent_stage.exists());
        assert_eq!(std::fs::read(&kept).unwrap(), b"previous staged bundle");
    }
    std::fs::write(&profile, &bytes).unwrap();
    std::fs::write(&archive, b"substituted archive").unwrap();
    assert!(stage_bundle(&result, &stage).is_err());
    assert!(stage_bundle(&result, &absent_stage).is_err());
    assert!(!absent_stage.exists());
    assert_eq!(std::fs::read(&kept).unwrap(), b"previous staged bundle");
    std::fs::write(&archive, b"original archive bytes").unwrap();
    stage_bundle(&result, &stage).unwrap();
    assert!(!kept.exists());
    result
        .network_profiles
        .as_ref()
        .unwrap()
        .verify_installed(&NativeBundleLayout::current().profiles_path(&stage.join("bundle")))
        .unwrap();
    assert_eq!(
        std::fs::read(stage.join("bundle.tar.gz")).unwrap(),
        b"original archive bytes"
    );
    assert_eq!(std::fs::read(profile).unwrap(), bytes);
}
