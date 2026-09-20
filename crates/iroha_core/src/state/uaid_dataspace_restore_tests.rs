//! Derived binding history from actual manifest/account MV publications.

use super::*;
use iroha_data_model::{
    account::AccountDetails,
    nexus::{AssetPermissionManifest, ManifestVersion},
};
use iroha_model_base::metadata::Metadata;

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn account(uaid: UniversalAccountId) -> AccountValue {
    AccountValue::new(AccountDetails::new(
        Metadata::default(),
        None,
        Some(uaid),
        Vec::new(),
    ))
}

fn manifest(uaid: UniversalAccountId, dataspace: DataSpaceId) -> SpaceDirectoryManifestSet {
    let mut record =
        crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(AssetPermissionManifest {
            version: ManifestVersion::default(),
            uaid,
            dataspace,
            issued_ms: 0,
            activation_epoch: 1,
            expiry_epoch: None,
            entries: Vec::new(),
        });
    record.lifecycle.mark_activated(1);
    let mut set = SpaceDirectoryManifestSet::default();
    set.upsert(record);
    set
}

fn fixture() -> (World, UniversalAccountId, AccountId) {
    let mut world = World::default();
    let uaid = UniversalAccountId::from_hash(Hash::new(b"derived UAID predecessor"));
    let account_id = iroha_test_samples::ALICE_ID.clone();
    world.accounts.insert(account_id.clone(), account(uaid));
    world.uaid_accounts.insert(uaid, account_id.clone());
    world
        .space_directory_manifests
        .insert(uaid, manifest(uaid, DataSpaceId::new(7)));
    assert_eq!(rebuild(&mut world).unwrap(), 1);
    (world, uaid, account_id)
}

fn cuts(
    world: &World,
) -> (
    BTreeMap<UniversalAccountId, UaidDataspaceBindings>,
    BTreeMap<UniversalAccountId, Option<UaidDataspaceBindings>>,
) {
    let snapshot = world.uaid_dataspaces.snapshot();
    (
        snapshot
            .current()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect(),
        snapshot
            .revert_map()
            .iter()
            .map(|(key, value)| (*key, value.clone()))
            .collect(),
    )
}

#[test]
fn removed_manifest_restores_prior_binding_and_absent_touch() {
    run(|| {
        let (mut world, uaid, account_id) = fixture();
        let never_present = UniversalAccountId::from_hash(Hash::new(b"absent touched UAID"));
        let mut block = world.block();
        block.space_directory_manifests.remove(uaid);
        block.space_directory_manifests.remove(never_present);
        block.commit();
        assert_eq!(rebuild(&mut world).unwrap(), 0);
        let retained = cuts(&world);
        assert!(retained.0.is_empty());
        assert!(
            retained.1[&uaid]
                .as_ref()
                .unwrap()
                .is_bound_to(DataSpaceId::new(7), &account_id)
        );
        assert_eq!(retained.1.get(&never_present), Some(&None));
        {
            let previous = world.block_and_revert();
            assert!(previous.space_directory_manifests.get(&uaid).is_some());
            assert!(
                previous
                    .uaid_dataspaces
                    .get(&uaid)
                    .unwrap()
                    .is_bound_to(DataSpaceId::new(7), &account_id)
            );
        }
        assert_eq!(
            cuts(&world),
            retained,
            "abandoning replacement preserves both derived cuts"
        );
        rebuild(&mut world).unwrap();
        assert_eq!(
            cuts(&world),
            retained,
            "startup and final restore passes are idempotent"
        );
    });
}

#[test]
fn account_and_manifest_changes_derive_both_uaid_identities() {
    run(|| {
        let (mut world, old, account_id) = fixture();
        let new = UniversalAccountId::from_hash(Hash::new(b"new account UAID"));
        let mut block = world.block();
        block.accounts.insert(account_id.clone(), account(new));
        block.uaid_accounts.remove(old);
        block.uaid_accounts.insert(new, account_id.clone());
        block.space_directory_manifests.remove(old);
        block
            .space_directory_manifests
            .insert(new, manifest(new, DataSpaceId::new(9)));
        block.commit();
        rebuild(&mut world).unwrap();
        let (current, undo) = cuts(&world);
        assert!(!current.contains_key(&old));
        assert!(current[&new].is_bound_to(DataSpaceId::new(9), &account_id));
        assert!(
            undo[&old]
                .as_ref()
                .unwrap()
                .is_bound_to(DataSpaceId::new(7), &account_id)
        );
        assert_eq!(undo.get(&new), Some(&None));
        let previous = world.block_and_revert();
        assert_eq!(
            previous.accounts.get(&account_id).unwrap().as_ref().uaid(),
            Some(&old)
        );
        assert!(
            previous
                .uaid_dataspaces
                .get(&old)
                .unwrap()
                .is_bound_to(DataSpaceId::new(7), &account_id)
        );
        assert!(previous.uaid_dataspaces.get(&new).is_none());
    });
}

#[test]
fn replacement_commit_retains_original_predecessor_and_drop_retains_tip() {
    run(|| {
        let (mut world, uaid, account_id) = fixture();
        let mut block = world.block();
        block
            .space_directory_manifests
            .insert(uaid, manifest(uaid, DataSpaceId::new(9)));
        block.commit();
        rebuild(&mut world).unwrap();
        let tip = cuts(&world);
        {
            let mut abandoned = world.block_and_revert();
            abandoned
                .space_directory_manifests
                .insert(uaid, manifest(uaid, DataSpaceId::new(11)));
            {
                let mut child = abandoned.transaction_without_telemetry(LaneConfig::default(), 0);
                child.rebuild_space_directory_bindings(uaid);
                child.apply();
            }
            assert!(
                abandoned
                    .uaid_dataspaces
                    .get(&uaid)
                    .unwrap()
                    .is_bound_to(DataSpaceId::new(11), &account_id)
            );
        }
        assert_eq!(cuts(&world), tip);
        {
            let mut replacement = world.block_and_revert();
            replacement
                .space_directory_manifests
                .insert(uaid, manifest(uaid, DataSpaceId::new(11)));
            {
                let mut child = replacement.transaction_without_telemetry(LaneConfig::default(), 0);
                child.rebuild_space_directory_bindings(uaid);
                child.apply();
            }
            replacement.commit();
        }
        rebuild(&mut world).unwrap();
        let current = world.uaid_dataspaces.view();
        assert!(
            current
                .get(&uaid)
                .unwrap()
                .is_bound_to(DataSpaceId::new(11), &account_id)
        );
        drop(current);
        let previous = world.block_and_revert();
        assert!(
            previous
                .uaid_dataspaces
                .get(&uaid)
                .unwrap()
                .is_bound_to(DataSpaceId::new(7), &account_id)
        );
        assert!(
            !previous
                .uaid_dataspaces
                .get(&uaid)
                .unwrap()
                .is_bound_to(DataSpaceId::new(9), &account_id)
        );
    });
}

#[test]
fn malformed_retained_manifest_refuses_before_derived_publication() {
    run(|| {
        let (mut world, uaid, _) = fixture();
        let retained = cuts(&world);
        let mut bad = manifest(uaid, DataSpaceId::new(7));
        let mut record = bad.remove(&DataSpaceId::new(7)).unwrap();
        record.manifest_hash = Hash::new(b"foreign canonical manifest hash");
        bad.upsert(record);
        let mut block = world.block();
        block.space_directory_manifests.insert(uaid, bad);
        block.commit();
        let mut block = world.block();
        block
            .space_directory_manifests
            .insert(uaid, manifest(uaid, DataSpaceId::new(9)));
        block.commit();
        // Current is valid; the actual retained previous record is malformed.
        assert!(snapshot_storage::manifest_set_matches_key(
            &uaid,
            world.space_directory_manifests.view().get(&uaid).unwrap()
        ));
        assert!(rebuild(&mut world).unwrap_err().contains("canonical hash"));
        assert_eq!(
            cuts(&world),
            retained,
            "failed predecessor validation publishes no derived map"
        );
    });
}

#[test]
fn derived_cache_contents_cannot_authorize_either_rebuilt_cut() {
    run(|| {
        let (mut world, uaid, account_id) = fixture();
        let expected = cuts(&world);
        let mut fake = UaidDataspaceBindings::default();
        fake.bind_account(DataSpaceId::new(99), account_id.clone());
        world.uaid_dataspaces = Storage::from_snapshot_parts(
            BTreeMap::from([(uaid, fake.clone())]),
            BTreeMap::from([(uaid, Some(fake))]),
        );
        rebuild(&mut world).unwrap();
        assert_eq!(cuts(&world), expected);
        assert!(
            world
                .uaid_dataspaces
                .view()
                .get(&uaid)
                .unwrap()
                .is_bound_to(DataSpaceId::new(7), &account_id)
        );
    });
}
