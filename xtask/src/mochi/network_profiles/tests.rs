//! Structural release preset controls; fixture keys establish no official Taira authority.

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
fn absent_release_input_and_arbitrary_override_refuse_before_output_or_build() {
    let root = tempfile::tempdir().unwrap();
    let retained = root.path().join("kept");
    std::fs::write(&retained, b"retained bundle").unwrap();
    assert!(select(root.path(), "release", None).is_err());
    assert!(validate_input("release", Some(Path::new("override.nrt"))).is_err());
    assert!(
        super::super::bundle_mochi(
            root.path(),
            "release",
            false,
            Some(Path::new("override.nrt"))
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&retained).unwrap(), b"retained bundle");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn release_selection_requires_taira_and_development_keeps_explicit_absence() {
    let root = tempfile::tempdir().unwrap();
    assert!(select(root.path(), "debug", None).unwrap().is_none());
    let empty = InstalledNetworkProfiles::new(Vec::new())
        .unwrap()
        .encode_installation()
        .unwrap();
    assert!(selected(&empty, Some("fixture-only-commit-evidence")).is_err());
    assert!(selected(&image("fixture"), Some("fixture-only-commit-evidence")).is_err());
    assert!(selected(b"untrusted response", None).is_err());
    let source = root.path().join("fixture.nrt");
    std::fs::write(&source, image("fixture")).unwrap();
    let selected = select(root.path(), "debug", Some(&source))
        .unwrap()
        .unwrap();
    assert_eq!(
        selected.provenance()["kind"].as_str(),
        Some("explicit_development_input")
    );
    assert_eq!(selected.provenance()["source_commit"], Value::Null);
}

#[test]
fn an_uncommitted_taira_named_artifact_cannot_establish_release_provenance() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(RELEASE_PROFILES);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = image("taira");
    std::fs::write(&path, &bytes).unwrap();
    // These fixture bytes are not the approved committed artifact. A valid label/key alone refuses.
    assert!(select(root.path(), "release", None).is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn exact_original_image_names_and_public_provenance_survive_staging() {
    let root = tempfile::tempdir().unwrap();
    let bytes = image("fixture");
    let selection = selected(&bytes, None).unwrap();
    selection.stage(root.path()).unwrap();
    selection.verify_installed(root.path()).unwrap();
    assert_eq!(
        std::fs::read(NativeBundleLayout::current().profiles_path(root.path())).unwrap(),
        bytes
    );
    selection.require_cli_names(br#"["fixture"]"#).unwrap();
    for bad in [
        br#"[]"#.as_slice(),
        br#"["taira"]"#,
        br#"["fixture","fixture"]"#,
        b"[",
        br#"["fixture"] {}"#,
    ] {
        assert!(selection.require_cli_names(bad).is_err());
    }
    assert!(
        selection
            .require_cli_names(&vec![b' '; MAX_INSTALLED_PROFILE_BYTES + 1])
            .is_err()
    );
    assert_eq!(
        selection.provenance()["sha256"].as_str(),
        Some(sha256_hex(&bytes).as_str())
    );
    let path = NativeBundleLayout::current().profiles_path(root.path());
    std::fs::write(&path, image("changed")).unwrap();
    assert!(selection.verify_installed(root.path()).is_err());
    std::fs::remove_file(&path).unwrap();
    assert!(selection.verify_installed(root.path()).is_err());
    assert!(!path.exists());
}

#[test]
fn matrix_cannot_claim_complete_qualification_or_lose_original_profile_custody() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("bundle");
    let selection = selected(&image("fixture"), None).unwrap();
    selection.stage(&bundle).unwrap();
    let manifest = bundle.join("manifest.json");
    std::fs::write(&manifest, b"{}").unwrap();
    let mut result = super::super::MochiBundleResult {
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
    super::super::update_bundle_matrix(&result, &matrix, true).unwrap();
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
    assert!(super::super::update_bundle_matrix(&result, &matrix, true).is_err());
    assert_eq!(std::fs::read(&matrix).unwrap(), original);
    result.network_profiles = None;
    result.profile = "release".into();
    assert!(super::super::run_bundle_smoke(&result).is_err());
    assert!(super::super::update_bundle_matrix(&result, &matrix, true).is_err());
    assert_eq!(std::fs::read(&matrix).unwrap(), original);
}

#[test]
fn staging_revalidates_original_profile_and_archive_before_replacing_outputs() {
    use super::super::{MochiBundleResult, archive_digest, stage_bundle};

    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("bundle");
    let bytes = image("fixture");
    let selection = selected(&bytes, None).unwrap();
    selection.stage(&bundle).unwrap();
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
        .verify_installed(&stage.join("bundle"))
        .unwrap();
    assert_eq!(
        std::fs::read(stage.join("bundle.tar.gz")).unwrap(),
        b"original archive bytes"
    );
    assert_eq!(std::fs::read(profile).unwrap(), bytes);
}
