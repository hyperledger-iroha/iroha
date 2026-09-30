//! Recovery storage refuses absent or uninitialized journals through the shipping library.
//! Keeping this integration target independent of `test-utils` also checks that production
//! archive and coordinator callers retain their complete journal-validation dependency chain.

#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt as _};

use iroha_core_zk::kagemusha_v1_state::{
    KagemushaLaneIdV1, KagemushaPendingRecoveryJournalsV1, KagemushaStateErrorV1,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, asset::AssetDefinitionId, nexus::AxtAssetIncarnationV1};

fn owner() -> (KagemushaLaneIdV1, AxtAssetIncarnationV1) {
    (
        KagemushaLaneIdV1 {
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"recovery-storage-library-dependency",
            ))),
            device_lane_id: [2; 32],
            asset: AssetDefinitionId::from_uuid_bytes([
                0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
                0xcd, 0x2f,
            ])
            .unwrap(),
            scale: 2,
        },
        AxtAssetIncarnationV1::try_from_bytes([3; 32]).unwrap(),
    )
}

#[test]
fn pending_recovery_never_initializes_missing_journals() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let coordinator = root.join("coordinator");
    let responses = root.join("responses");
    let (lane, incarnation) = owner();

    assert!(matches!(
        KagemushaPendingRecoveryJournalsV1::open_existing(
            &coordinator,
            &responses,
            &lane,
            incarnation,
            1 << 20,
        ),
        Err(KagemushaStateErrorV1::RecoveryMaterial(_))
    ));
    assert!(!coordinator.exists());
    assert!(!responses.exists());
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn pending_recovery_preserves_an_uninitialized_journal_and_releases_its_lock() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let coordinator = root.join("coordinator");
    fs::create_dir(&coordinator).unwrap();
    fs::set_permissions(&coordinator, fs::Permissions::from_mode(0o700)).unwrap();
    let journal = coordinator.join("operations.norito.wal");
    fs::write(&journal, []).unwrap();
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o600)).unwrap();
    let responses = root.join("responses");
    let (lane, incarnation) = owner();

    for _ in 0..2 {
        let error = match KagemushaPendingRecoveryJournalsV1::open_existing(
            &coordinator,
            &responses,
            &lane,
            incarnation,
            1 << 20,
        ) {
            Ok(_) => panic!("an empty journal cannot establish recovery material"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            KagemushaStateErrorV1::RecoveryMaterial(ref reason)
                if reason == "coordinator operation journal integrity failed"
        ));
        assert!(fs::read(&journal).unwrap().is_empty());
        assert_eq!(fs::read_dir(&coordinator).unwrap().count(), 1);
        assert!(!responses.exists());
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(journal)
        .unwrap();
    file.try_lock()
        .expect("failed recovery relinquishes its descriptor lock");
}
