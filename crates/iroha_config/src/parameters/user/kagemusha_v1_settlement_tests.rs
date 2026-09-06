//! KAGEMUSHA V1 settlement configuration tests.

use super::*;

#[test]
fn kagemusha_v1_parse_initializes_empty_runtime_custody() {
    let mut emitter = Emitter::new();
    let actual = Kagemusha::default().parse(&mut emitter);

    assert!(emitter.into_result().is_ok());
    assert!(actual.reserve_accounts.is_empty());
    assert!(actual.proof_release.is_none());
}

#[test]
fn kagemusha_v1_release_paths_are_all_or_none() {
    let mut partial = Kagemusha::default();
    partial.release_manifest_path = Some(PathBuf::from("release.nrt"));
    let mut emitter = Emitter::new();
    let parsed = partial.parse(&mut emitter);
    assert!(emitter.into_result().is_err());
    assert!(parsed.proof_release.is_none());

    let mut complete = Kagemusha::default();
    complete.release_manifest_path = Some(PathBuf::from("release.nrt"));
    complete.validation_receipt_path = Some(PathBuf::from("receipt.nrt"));
    complete.authority_policy_path = Some(PathBuf::from("authority.nrt"));
    complete.release_attestation_path = Some(PathBuf::from("attestation.nrt"));
    complete.recursive_profile_path = Some(PathBuf::from("profile.json"));
    complete.artifact_directory = Some(PathBuf::from("artifacts"));
    let mut emitter = Emitter::new();
    let parsed = complete.parse(&mut emitter);
    assert!(emitter.into_result().is_ok());
    let release = parsed.proof_release.expect("complete release is retained");
    assert_eq!(release.manifest, PathBuf::from("release.nrt"));
    assert_eq!(release.artifact_directory, PathBuf::from("artifacts"));
}
