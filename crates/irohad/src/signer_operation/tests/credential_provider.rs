//! Supervisor-credential provider tests against the existing reservation model.

use super::*;
use crate::signer_operation::credential_provider::SoftwareCredentialSignerKeyOperationProviderV1;
use iroha_crypto::ExposedPrivateKey;
use std::{
    fs,
    os::unix::fs::{PermissionsExt as _, symlink},
    path::PathBuf,
};
use zeroize::Zeroizing;

/// Write an owner-only canonical test credential below the checkout's target directory.
pub(super) fn credential(seed: u8) -> (tempfile::TempDir, PathBuf) {
    let parent = std::env::current_dir()
        .expect("checkout directory")
        .join("target");
    fs::create_dir_all(&parent).expect("target directory");
    let directory = tempfile::Builder::new()
        .prefix(".signer-credential-test-")
        .tempdir_in(parent)
        .expect("private target directory");
    let path = directory.path().join("private-key.credential");
    let literal = Zeroizing::new(
        ExposedPrivateKey(key(seed).private_key().clone())
            .try_to_multihash_string()
            .expect("canonical private key"),
    );
    let mut bytes = Zeroizing::new(literal.as_bytes().to_vec());
    bytes.push(b'\n');
    fs::write(&path, bytes.as_slice()).expect("write private credential");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .expect("owner-only private credential");
    (directory, path)
}

fn load(
    fixture: &Fixture,
    path: &std::path::Path,
) -> Result<SoftwareCredentialSignerKeyOperationProviderV1, SignerOperationErrorV1> {
    SoftwareCredentialSignerKeyOperationProviderV1::load_from_supervisor_credential(
        path,
        fixture.coordinator.binding.clone(),
        fixture.coordinator.record.clone(),
        fixture.coordinator.trust.clone(),
        fixture.source.clone(),
    )
}

#[test]
fn credential_provider_signs_only_with_the_exact_enrolled_software_key() {
    let fixture = fixture();
    let (_directory, path) = credential(0x21);
    let coordinator = SignerOperationCoordinatorV1::from_software_supervisor_credential(
        &path,
        fixture.coordinator.binding.clone(),
        fixture.coordinator.record.clone(),
        fixture.coordinator.trust.clone(),
        Some(fixture.source.clone()),
    )
    .expect("active owner-only credential and coordinator");
    let mut operation = coordinator
        .begin(intent(SignerOperationActionV1::Sign))
        .expect("exclusive reservation");
    stage(&mut operation);
    let completed = operation.finish(commitment()).expect("durable completion");
    assert!(
        completed
            .signature(SignerKeyOperationPurposeV1::RolePayload)
            .is_some()
    );
    assert!(
        fixture
            .source
            .state
            .lock()
            .expect("source lock")
            .reserved_reads
            >= 12
    );

    let (_wrong_directory, wrong_path) = credential(0x22);
    assert!(matches!(
        load(&fixture, &wrong_path),
        Err(SignerOperationErrorV1::ProviderUnavailable)
    ));
    let link = path.with_extension("link");
    symlink(&path, &link).expect("test credential symlink");
    assert!(matches!(
        load(&fixture, &link),
        Err(SignerOperationErrorV1::ProviderUnavailable)
    ));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
        .expect("make test credential publicly readable");
    assert!(matches!(
        load(&fixture, &path),
        Err(SignerOperationErrorV1::ProviderUnavailable)
    ));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .expect("restore owner-only test credential");
    fs::write(&path, b"ed25519:noncanonical\n").expect("replace test credential");
    assert!(matches!(
        load(&fixture, &path),
        Err(SignerOperationErrorV1::ProviderUnavailable)
    ));
}

#[test]
fn software_coordinator_requires_configured_state_before_credential_access() {
    let fixture = fixture();
    let (_directory, path) = credential(0x21);
    fs::remove_file(&path).expect("remove credential before absent-source check");
    assert!(matches!(
        SignerOperationCoordinatorV1::from_software_supervisor_credential(
            &path,
            fixture.coordinator.binding.clone(),
            fixture.coordinator.record.clone(),
            fixture.coordinator.trust.clone(),
            None,
        ),
        Err(SignerOperationErrorV1::StateUnavailable)
    ));
}

#[test]
fn credential_provider_rechecks_the_exact_reservation_before_key_use() {
    let fixture = fixture();
    let (_directory, path) = credential(0x21);
    let provider = load(&fixture, &path).expect("active owner-only credential");
    let operation = fixture
        .coordinator
        .begin(intent(SignerOperationActionV1::Sign))
        .expect("exclusive reservation");
    fixture
        .source
        .state
        .lock()
        .expect("source lock")
        .mutate(Mutation::LostReservation);
    let request = SignerKeyOperationRequestV1 {
        check: operation.check(),
        purpose: SignerKeyOperationPurposeV1::RolePayload,
        ordinal: 1,
        message: b"validated-role-payload",
    };
    assert!(matches!(
        provider.sign(&request),
        Err(SignerOperationErrorV1::ReservationConflict)
    ));
}

#[test]
fn credential_provider_rejects_newly_revoked_custody_before_key_use() {
    let fixture = fixture();
    let (_directory, path) = credential(0x21);
    let provider = load(&fixture, &path).expect("active owner-only credential");
    let operation = fixture
        .coordinator
        .begin(intent(SignerOperationActionV1::Sign))
        .expect("exclusive reservation");
    fixture
        .source
        .state
        .lock()
        .expect("source lock")
        .mutate(Mutation::SignerRevoked);
    let request = SignerKeyOperationRequestV1 {
        check: operation.check(),
        purpose: SignerKeyOperationPurposeV1::RolePayload,
        ordinal: 1,
        message: b"validated-role-payload",
    };
    assert!(matches!(
        provider.sign(&request),
        Err(SignerOperationErrorV1::Custody(
            sorafs_manifest::signer::custody::SignerCustodyErrorV1::Revoked
        ))
    ));
}

#[test]
fn credential_provider_rejects_a_changed_active_record_before_key_use() {
    let fixture = fixture();
    let (_directory, path) = credential(0x21);
    let provider = load(&fixture, &path).expect("active owner-only credential");
    let operation = fixture
        .coordinator
        .begin(intent(SignerOperationActionV1::Sign))
        .expect("exclusive reservation");
    fixture
        .source
        .state
        .lock()
        .expect("source lock")
        .mutate(Mutation::Record);
    let request = SignerKeyOperationRequestV1 {
        check: operation.check(),
        purpose: SignerKeyOperationPurposeV1::RolePayload,
        ordinal: 1,
        message: b"validated-role-payload",
    };
    assert!(matches!(
        provider.sign(&request),
        Err(SignerOperationErrorV1::Custody(_))
    ));
}
