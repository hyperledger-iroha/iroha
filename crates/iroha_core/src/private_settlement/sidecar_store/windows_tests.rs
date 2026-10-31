//! Native APS storage, restart and original-owner mutation controls.

use super::*;
use std::{
    io::{Seek as _, Write as _},
    process::Command,
};

fn root() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("restricted-sidecars");
    (temporary, path)
}

fn bytes() -> (Hash, Vec<u8>) {
    let fixture = super::super::tests::sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let record = DurablePrivateSettlementSidecarV1::new(fixture.sidecar);
    (digest, norito::encode_canonical(&record).unwrap())
}

fn grant(path: &Path, access: &str) {
    let status = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", access])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn native_store_create_restart_lifecycle_and_retirement_preserve_reservations() {
    let (temporary, path) = root();
    let fixture = super::super::tests::sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let expiry = fixture.sidecar.manifest.expiry_height;
    let retention = fixture
        .sidecar
        .payload
        .availability
        .body
        .retention_until_height;
    let config = PrivateSettlementSidecarStoreConfigV1::default();
    let store = PrivateSettlementFileSidecarStoreV1::open(&path, config).unwrap();
    assert_eq!(
        store.store(fixture.sidecar.clone()).unwrap(),
        PrivateSettlementSidecarStoreOutcomeV1::Stored
    );
    let evidence = store.staged_lock_evidence_v1().unwrap();
    assert!(evidence.counts.total > 0);
    assert_eq!(
        store.prune(retention + 1).unwrap(),
        0,
        "nonterminal records are never pruned"
    );
    drop(store);
    let store = PrivateSettlementFileSidecarStoreV1::open(&path, config).unwrap();
    assert_eq!(store.staged_lock_evidence_v1().unwrap(), evidence);
    assert_eq!(
        store.store(fixture.sidecar).unwrap(),
        PrivateSettlementSidecarStoreOutcomeV1::AlreadyStored
    );
    assert!(
        store
            .expire_at_authoritative_height(digest, expiry + 1)
            .unwrap()
    );
    assert_eq!(store.staged_lock_evidence_v1().unwrap().counts.total, 0);
    drop(store);
    let store = PrivateSettlementFileSidecarStoreV1::open(&path, config).unwrap();
    assert_eq!(store.prune(retention.max(expiry) + 2).unwrap(), 1);
    drop(store);
    let store = PrivateSettlementFileSidecarStoreV1::open(&path, config).unwrap();
    assert!(store.state.lock().unwrap().index.is_empty());
    drop(store);
    drop(temporary);
}

#[test]
fn native_writer_lock_excludes_aliases_and_name_removal() {
    let (_temporary, path) = root();
    let store = NativeStore::open(&path).unwrap();
    assert!(NativeStore::open(&path).is_err());
    let alias = fs::canonicalize(&path).unwrap();
    assert!(NativeStore::open(&alias).is_err());
    assert!(fs::remove_file(path.join(SIDECAR_WRITER_LOCK_FILE_V1)).is_err());
    assert!(File::open(path.join(SIDECAR_WRITER_LOCK_FILE_V1)).is_err());
    store.revalidate().unwrap();
}

#[test]
fn native_prepublication_retry_reads_the_same_held_predecessor() {
    let (_temporary, path) = root();
    let store = NativeStore::open(&path).unwrap();
    let (digest, payload) = bytes();
    let name = sidecar_file_name_v1(digest);
    store.persist(&name, &payload, false).unwrap();
    let failure = store
        .persist_observed(&name, &payload, true, |step| {
            if step == AtomicWriteStep::Publish {
                Err(io::Error::other("before publication"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert!(!failure.committed);
    let identity = match store.pending.lock().unwrap().as_ref().unwrap() {
        Pending::Publication(p) => FileIdentity::of(p.original.as_ref().unwrap()).unwrap(),
        Pending::Removal(_) => panic!("publication expected"),
    };
    assert_eq!(
        store.read(&name, |bytes| Ok(bytes.to_vec())).unwrap(),
        payload
    );
    assert!(store.persist(&name, b"different retry", true).is_err());
    store
        .persist_observed(&name, &payload, true, |step| {
            if step == AtomicWriteStep::Published {
                let held = store
                    .owner
                    .open_regular_optional(&name)?
                    .ok_or_else(changed)?;
                assert_eq!(FileIdentity::of(&held)?, identity);
            }
            Ok(())
        })
        .unwrap();
    assert!(store.pending.lock().unwrap().is_none());
}

#[test]
fn native_postpublication_failure_is_committed_and_keeps_both_originals() {
    let (_temporary, path) = root();
    let store = NativeStore::open(&path).unwrap();
    let (digest, payload) = bytes();
    let name = sidecar_file_name_v1(digest);
    store.persist(&name, &payload, false).unwrap();
    let failure = store
        .persist_observed(&name, &payload, true, |step| {
            if step == AtomicWriteStep::Published {
                Err(io::Error::other("after publication"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert!(failure.committed);
    assert!(
        !store
            .persist("different.aps1", &payload, false)
            .unwrap_err()
            .committed,
        "a refused unrelated request did not publish its own record"
    );
    {
        let pending = store.pending.lock().unwrap();
        let Pending::Publication(p) = pending.as_ref().unwrap() else {
            panic!("publication")
        };
        assert!(p.published);
        store.check_pending(pending.as_ref().unwrap()).unwrap();
        store
            .owner
            .require_retired_private_journal(p.predecessor.as_ref().unwrap())
            .unwrap();
    }
    // The common lifecycle poisons after committed failure; a restart reads canonical state.
    drop(store);
    let reopened = NativeStore::open(&path).unwrap();
    let state = reopened
        .recover(PrivateSettlementSidecarStoreConfigV1::default())
        .unwrap();
    assert!(state.index.contains_key(&digest));
}

#[test]
fn native_retirement_retries_postclose_barrier_without_reopening_name() {
    let (_temporary, path) = root();
    let store = NativeStore::open(&path).unwrap();
    let (digest, payload) = bytes();
    let name = sidecar_file_name_v1(digest);
    store.persist(&name, &payload, false).unwrap();
    for _ in 0..2 {
        let failure = store
            .remove_observed(&name, false, |step| {
                if step == AtomicWriteStep::SyncDirectory {
                    Err(io::Error::other("barrier"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(failure.committed);
        assert!(!path.join(&name).exists());
        assert!(
            !store.remove("different.aps1").unwrap_err().committed,
            "a refused unrelated removal did not dispose its own record"
        );
        let pending = store.pending.lock().unwrap();
        let Pending::Removal(p) = pending.as_ref().unwrap() else {
            panic!("removal")
        };
        assert!(p.disposed && p.closed && p.original.is_none());
    }
    store.remove(&name).unwrap();
    assert!(store.pending.lock().unwrap().is_none());
}

#[test]
fn native_recovery_cleans_only_known_private_stages_and_rejects_corruption() {
    let (_temporary, path) = root();
    let store = NativeStore::open(&path).unwrap();
    let (digest, payload) = bytes();
    let name = sidecar_file_name_v1(digest);
    store.persist(&name, &payload, false).unwrap();
    let stage = format!("{name}{SIDECAR_TEMP_EXTENSION_V1}");
    let mut file = None;
    store
        .temporary_owner
        .create_private_journal_into(&stage, &mut file)
        .unwrap();
    file.as_mut().unwrap().write_all(b"interrupted").unwrap();
    file.as_ref().unwrap().sync_all().unwrap();
    drop(file);
    drop(store);
    let store = NativeStore::open(&path).unwrap();
    assert_eq!(
        store
            .recover(PrivateSettlementSidecarStoreConfigV1::default())
            .unwrap()
            .index
            .len(),
        1
    );
    assert!(!store.temporary.path().join(stage).exists());
    let mut file = store
        .owner
        .open_private_journal_optional(&name)
        .unwrap()
        .unwrap();
    file.seek(SeekFrom::End(0)).unwrap();
    file.write_all(&[0]).unwrap();
    file.sync_all().unwrap();
    drop(file);
    drop(store);
    let store = NativeStore::open(&path).unwrap();
    assert!(matches!(
        store.recover(PrivateSettlementSidecarStoreConfigV1::default()),
        Err(Error::Corrupt)
    ));
}

#[test]
fn native_recovery_reconciles_exact_provisional_promotion_before_capacity() {
    let (_temporary, path) = root();
    let fixture = super::super::tests::sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let provisional = DurablePrivateSettlementProvisionalSidecarV1::new(
        super::super::tests::provisional_material_fixture(&fixture),
        fixture.sidecar.stored_at_height,
    );
    let final_record = DurablePrivateSettlementSidecarV1::new(fixture.sidecar);
    let store = NativeStore::open(&path).unwrap();
    let final_bytes = norito::encode_canonical(&final_record).unwrap();
    store
        .persist(
            &provisional_file_name_v1(digest),
            &norito::encode_canonical(&provisional).unwrap(),
            false,
        )
        .unwrap();
    store
        .persist(&sidecar_file_name_v1(digest), &final_bytes, false)
        .unwrap();
    drop(store);
    let store = NativeStore::open(&path).unwrap();
    let config = PrivateSettlementSidecarStoreConfigV1::new(1, final_bytes.len() as u64).unwrap();
    let state = store.recover(config).unwrap();
    assert_eq!(state.index.len(), 1);
    assert!(state.provisional_index.is_empty());
    assert_eq!(state.canonical_bytes, final_bytes.len() as u64);
    assert!(!path.join(provisional_file_name_v1(digest)).exists());
    assert!(!state.pool_reservations.is_empty());
}

#[test]
fn native_capacity_and_filename_digest_checks_remain_common() {
    let (_temporary, path) = root();
    let store = NativeStore::open(&path).unwrap();
    let (digest, payload) = bytes();
    store
        .persist(&sidecar_file_name_v1(digest), &payload, false)
        .unwrap();
    let config = PrivateSettlementSidecarStoreConfigV1::new(1, payload.len() as u64 - 1).unwrap();
    assert!(matches!(
        store.recover(config),
        Err(Error::CapacityExceeded)
    ));
    let other = sidecar_file_name_v1(Hash::prehashed([0x99; Hash::LENGTH]));
    fs::rename(path.join(sidecar_file_name_v1(digest)), path.join(other)).unwrap();
    assert!(matches!(
        store.recover(PrivateSettlementSidecarStoreConfigV1::default()),
        Err(Error::Corrupt)
    ));
}

#[test]
fn native_acl_hardlink_and_unknown_entries_are_refused_without_repair() {
    for mode in 0..3 {
        let (_temporary, path) = root();
        let store = NativeStore::open(&path).unwrap();
        let (digest, payload) = bytes();
        let name = sidecar_file_name_v1(digest);
        store.persist(&name, &payload, false).unwrap();
        match mode {
            0 => grant(&path.join(&name), "*S-1-1-0:(R)"),
            1 => fs::hard_link(path.join(&name), path.join("foreign-link")).unwrap(),
            _ => fs::write(path.join("unknown"), b"untouched").unwrap(),
        }
        assert!(
            store
                .recover(PrivateSettlementSidecarStoreConfigV1::default())
                .is_err()
        );
        assert_eq!(fs::read(path.join(&name)).unwrap(), payload);
    }
}

#[test]
fn native_private_parent_drift_refuses_before_publication_even_when_observer_returns_ok() {
    for temp in [false, true] {
        let (_temporary, path) = root();
        let store = NativeStore::open(&path).unwrap();
        let (digest, payload) = bytes();
        let name = sidecar_file_name_v1(digest);
        store.persist(&name, &payload, false).unwrap();
        let failure = store
            .persist_observed(&name, &payload, true, |step| {
                if step == AtomicWriteStep::Publish {
                    grant(
                        if temp {
                            store.temporary.path()
                        } else {
                            store.root.path()
                        },
                        "*S-1-1-0:(R)",
                    );
                }
                Ok(())
            })
            .unwrap_err();
        assert!(!failure.committed);
        assert_eq!(fs::read(path.join(name)).unwrap(), payload);
    }
}

#[test]
fn native_record_extent_and_reparse_leaf_refuse_without_privilege_skips() {
    let (_temporary, path) = root();
    let store = NativeStore::open(&path).unwrap();
    let (digest, payload) = bytes();
    let name = sidecar_file_name_v1(digest);
    store.persist(&name, &payload, false).unwrap();
    let file = store
        .owner
        .open_private_journal_optional(&name)
        .unwrap()
        .unwrap();
    file.set_len(PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1 + 1)
        .unwrap();
    drop(file);
    assert!(matches!(store.read(&name, |_| Ok(())), Err(Error::Corrupt)));
    store
        .remove(&name)
        .expect_err("oversized original cannot be retired as a valid record");
    drop(store);
    fs::remove_file(path.join(&name)).unwrap();
    let target = path.parent().unwrap().join("junction-target");
    fs::create_dir(&target).unwrap();
    let status = Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(path.join(&name))
        .arg(&target)
        .status()
        .unwrap();
    assert!(status.success());
    let store = NativeStore::open(&path).unwrap();
    assert!(store.read(&name, |_| Ok(())).is_err());
    assert!(target.is_dir());
}

#[test]
fn native_public_promotion_retries_prepublication_failure_with_provisional_read() {
    let (_temporary, path) = root();
    let fixture = super::super::tests::sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let store = PrivateSettlementFileSidecarStoreV1::open(
        &path,
        PrivateSettlementSidecarStoreConfigV1::default(),
    )
    .unwrap();
    store
        .store_provisional(
            super::super::tests::provisional_material_fixture(&fixture),
            fixture.sidecar.stored_at_height,
        )
        .unwrap();
    *store.native.next_failure.lock().unwrap() = Some(AtomicWriteStep::Publish);
    assert_eq!(store.promote(fixture.sidecar.clone()), Err(Error::Backend));
    assert!(!store.state.lock().unwrap().poisoned);
    assert!(path.join(provisional_file_name_v1(digest)).exists());
    assert!(!path.join(sidecar_file_name_v1(digest)).exists());
    assert_eq!(
        store.promote(fixture.sidecar).unwrap(),
        PrivateSettlementSidecarStoreOutcomeV1::Stored
    );
    assert!(!path.join(provisional_file_name_v1(digest)).exists());
    assert!(store.native.pending.lock().unwrap().is_none());
    assert!(store.state.lock().unwrap().index.contains_key(&digest));
}

#[test]
fn native_public_promotion_committed_failure_poison_is_reconciled_on_restart() {
    let (_temporary, path) = root();
    let fixture = super::super::tests::sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let config = PrivateSettlementSidecarStoreConfigV1::default();
    let store = PrivateSettlementFileSidecarStoreV1::open(&path, config).unwrap();
    store
        .store_provisional(
            super::super::tests::provisional_material_fixture(&fixture),
            fixture.sidecar.stored_at_height,
        )
        .unwrap();
    *store.native.next_failure.lock().unwrap() = Some(AtomicWriteStep::Published);
    assert_eq!(store.promote(fixture.sidecar.clone()), Err(Error::Backend));
    assert!(store.state.lock().unwrap().poisoned);
    assert_eq!(store.promote(fixture.sidecar.clone()), Err(Error::Backend));
    assert!(path.join(sidecar_file_name_v1(digest)).exists());
    assert!(path.join(provisional_file_name_v1(digest)).exists());
    drop(store);
    let store = PrivateSettlementFileSidecarStoreV1::open(&path, config).unwrap();
    assert_eq!(
        store.promote(fixture.sidecar).unwrap(),
        PrivateSettlementSidecarStoreOutcomeV1::AlreadyStored
    );
    assert!(!path.join(provisional_file_name_v1(digest)).exists());
}
