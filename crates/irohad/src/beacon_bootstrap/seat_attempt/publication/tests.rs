//! Real path failures and immutable-source retry for the shared phase writer.

use super::*;
use std::{
    fs,
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
};

fn directory() -> (tempfile::TempDir, Directory) {
    let temporary = tempfile::Builder::new()
        .prefix(".beacon-phase-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let directory = Directory::open(&fs::canonicalize(temporary.path()).unwrap()).unwrap();
    (temporary, directory)
}

#[test]
fn refused_phase_publication_retains_exact_source_before_original_destination_exists() {
    let (temporary, directory) = directory();
    let path = temporary.path().join("deliveries.norito");
    let original = [0x51; 117];
    fs::write(&path, b"unrelated existing output").unwrap();
    let mut slot = PhasePublication::new(PhaseFile::Deliveries);
    let ExportError::Io(error) = slot.publish(&directory, &original).unwrap_err() else {
        panic!("actual exclusive-create refusal");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(slot.source.as_ref().unwrap().matches(&original));
    assert!(!slot.complete());
    assert_eq!(fs::read(&path).unwrap(), b"unrelated existing output");
    fs::remove_file(&path).unwrap();
    slot.publish(&directory, &original).unwrap();
    let inode = fs::metadata(&path).unwrap().ino();
    assert!(slot.complete());
    assert_eq!(fs::read(&path).unwrap(), original);
    slot.publish(&directory, &original).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
}

#[test]
fn phase_retry_rejects_same_allocation_changed_bytes_and_equal_foreign_allocation() {
    for replace_with_equal_copy in [false, true] {
        let (temporary, directory) = directory();
        let path = temporary.path().join("acceptances.norito");
        let mut original = [0x73; 117];
        let foreign = original;
        let mut slot = PhasePublication::new(PhaseFile::Acceptances);
        slot.publish(&directory, &original).unwrap();
        let inode = fs::metadata(&path).unwrap().ino();
        let candidate: &[u8] = if replace_with_equal_copy {
            &foreign
        } else {
            original[116] ^= 1;
            &original
        };
        assert!(matches!(
            slot.publish(&directory, candidate),
            Err(ExportError::Custody)
        ));
        assert!(slot.terminal_custody_failure);
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        assert_eq!(fs::read(&path).unwrap(), [0x73; 117]);
        assert!(matches!(
            slot.publish(&directory, &original),
            Err(ExportError::Custody)
        ));
    }
}

#[test]
fn every_phase_uses_its_exact_name_and_generation_intent_is_owner_private() {
    let (temporary, directory) = directory();
    let original = [0xA9; 73];
    for phase in [
        PhaseFile::GenerationIntent,
        PhaseFile::Publication,
        PhaseFile::Deliveries,
        PhaseFile::Acceptances,
    ] {
        let mut slot = PhasePublication::new(phase);
        slot.publish(&directory, &original).unwrap();
        let path = temporary.path().join(phase.name());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(slot.complete());
        if phase == PhaseFile::GenerationIntent {
            assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
        }
    }
}

#[test]
fn original_public_input_and_native_proof_roles_have_distinct_private_names_and_completed_source_hashes()
 {
    let (temporary, directory) = directory();
    for (index, phase) in [
        PhaseFile::CommitmentsInput,
        PhaseFile::DeliveriesInput,
        PhaseFile::SessionInput,
        PhaseFile::CommitmentsProof,
        PhaseFile::DeliveriesProof,
        PhaseFile::SessionProof,
    ]
    .into_iter()
    .enumerate()
    {
        let original = [u8::try_from(index).unwrap() + 1; 73];
        let mut slot = PhasePublication::new(phase);
        assert!(matches!(slot.complete_hash(), Err(ExportError::Phase)));
        slot.publish(&directory, &original).unwrap();
        let path = temporary.path().join(phase.name());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
        assert_eq!(
            slot.complete_hash().unwrap(),
            <[u8; 32]>::from(Hash::new(original))
        );
        let inode = fs::metadata(&path).unwrap().ino();
        slot.publish(&directory, &original).unwrap();
        assert_eq!(fs::metadata(path).unwrap().ino(), inode);
    }
}
