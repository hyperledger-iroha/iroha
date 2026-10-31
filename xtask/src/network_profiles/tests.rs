//! Structural release preset controls and exact installed discovery selections.

use super::*;
use iroha_deploy::{bootstrap::InstalledNetworkProfile, managed::NativeBundleLayout};

fn image(name: &str) -> Vec<u8> {
    InstalledNetworkProfiles::new(vec![
        InstalledNetworkProfile::new(
            name.into(),
            "https://fixture.invalid/",
            "fixture-chain".into(),
            369,
        )
        .unwrap(),
    ])
    .unwrap()
    .encode_installation()
    .unwrap()
}

fn taira_image() -> Vec<u8> {
    InstalledNetworkProfiles::new(vec![
        InstalledNetworkProfile::new(
            "taira".into(),
            "https://taira.sora.org/",
            "fc56984b-2be7-431d-840e-21514d1883f0".into(),
            369,
        )
        .unwrap(),
    ])
    .unwrap()
    .encode_installation()
    .unwrap()
}

#[test]
fn checked_in_taira_profile_selects_native_discovery() {
    let bytes = std::fs::read(crate::workspace_root().join(RELEASE_PROFILES)).unwrap();
    assert_eq!(bytes, taira_image());
    let profiles = InstalledNetworkProfiles::from_installation_bytes(&bytes).unwrap();
    let taira = profiles.select("taira").unwrap();
    assert_eq!(taira.torii_root().as_str(), "https://taira.sora.org/");
    assert_eq!(taira.chain_id(), "fc56984b-2be7-431d-840e-21514d1883f0");
    assert_eq!(taira.account_chain_discriminant(), 369);
    assert_eq!(profiles.names().collect::<Vec<_>>(), ["taira"]);
}

#[test]
#[ignore = "explicit maintenance command: regenerate the checked-in native discovery preset"]
fn regenerate_developer_network_profiles() {
    std::fs::write(
        crate::workspace_root().join(RELEASE_PROFILES),
        taira_image(),
    )
    .unwrap();
    checked_in_taira_profile_selects_native_discovery();
}

#[test]
fn absent_release_input_and_arbitrary_override_refuse_before_output_or_build() {
    let root = tempfile::tempdir().unwrap();
    let retained = root.path().join("kept");
    std::fs::write(&retained, b"retained bundle").unwrap();
    assert!(select(root.path(), "release", None).is_err());
    assert!(validate_input("release", Some(Path::new("override.nrt"))).is_err());
    assert!(
        crate::mochi::bundle_mochi(
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
    assert!(require_for_profile("release", Some(&selected)).is_err());
    assert!(require_for_profile("release", None).is_err());
    require_for_profile("debug", Some(&selected)).unwrap();
    require_for_profile("debug", None).unwrap();
}

#[test]
fn an_uncommitted_taira_named_artifact_cannot_establish_release_provenance() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(RELEASE_PROFILES);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = image("taira");
    std::fs::write(&path, &bytes).unwrap();
    // These fixture bytes are not the committed source artifact. A matching label alone refuses.
    assert!(select(root.path(), "release", None).is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn exact_original_image_names_and_public_provenance_survive_staging() {
    let root = tempfile::tempdir().unwrap();
    let bytes = image("fixture");
    let selection = selected(&bytes, None).unwrap();
    selection
        .stage(&NativeBundleLayout::current().profiles_path(root.path()))
        .unwrap();
    selection
        .verify_installed(&NativeBundleLayout::current().profiles_path(root.path()))
        .unwrap();
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
        Some(hex::encode(Sha256::digest(&bytes)).as_str())
    );
    let path = NativeBundleLayout::current().profiles_path(root.path());
    std::fs::write(&path, image("changed")).unwrap();
    assert!(
        selection
            .verify_installed(&NativeBundleLayout::current().profiles_path(root.path()))
            .is_err()
    );
    std::fs::remove_file(&path).unwrap();
    assert!(
        selection
            .verify_installed(&NativeBundleLayout::current().profiles_path(root.path()))
            .is_err()
    );
    assert!(!path.exists());
}
