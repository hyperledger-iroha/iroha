//! Space Directory manifest publication flow.

use iroha_data_model::{
    account::AccountId,
    events::data::space_directory::{
        SpaceDirectoryEvent, SpaceDirectoryManifestActivated, SpaceDirectoryManifestRevoked,
    },
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        space_directory::{
            ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest,
            RevokeSpaceDirectoryManifest,
        },
    },
    nexus::{DataSpaceId, UniversalAccountId},
    permission::{Permission, Permissions},
};
use iroha_executor_data_model::permission::nexus::CanPublishSpaceDirectoryManifest;

use super::*;
use crate::{nexus::space_directory::SpaceDirectoryManifestRecord, state::StateTransaction};

impl Execute for PublishSpaceDirectoryManifest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let manifest = self.manifest;
        let dataspace = manifest.dataspace;
        ensure_known_dataspace(state_transaction, dataspace)?;
        if !has_publish_permission(state_transaction, authority, dataspace) {
            return Err(InstructionExecutionError::InvariantViolation(
                "not permitted: CanPublishSpaceDirectoryManifest".into(),
            ));
        }

        let uaid = manifest.uaid;
        let activation_epoch = manifest.activation_epoch;
        let expiry_epoch = manifest.expiry_epoch;
        let mut record = SpaceDirectoryManifestRecord::new(manifest);
        record.lifecycle.mark_activated(activation_epoch);
        let manifest_hash = record.manifest_hash;
        upsert_manifest(state_transaction, uaid, record);
        state_transaction.rebuild_space_directory_bindings(uaid);
        state_transaction.refresh_axt_policies_from_directory();
        state_transaction
            .world
            .emit_events(Some(SpaceDirectoryEvent::ManifestActivated(
                SpaceDirectoryManifestActivated {
                    dataspace,
                    uaid,
                    manifest_hash,
                    activation_epoch,
                    expiry_epoch,
                },
            )));
        #[cfg(feature = "telemetry")]
        {
            state_transaction
                .telemetry
                .record_space_directory_revision(dataspace);
        }
        Ok(())
    }
}

impl Execute for ExpireSpaceDirectoryManifest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let dataspace = self.dataspace;
        ensure_known_dataspace(state_transaction, dataspace)?;
        if !has_publish_permission(state_transaction, authority, dataspace) {
            return Err(InstructionExecutionError::InvariantViolation(
                "not permitted: CanPublishSpaceDirectoryManifest".into(),
            ));
        }

        let uaid = self.uaid;
        state_transaction
            .world
            .expire_space_directory_manifest_record(
                uaid,
                dataspace,
                self.expired_epoch,
                &state_transaction.nexus.lane_config,
                state_transaction.axt_current_slot(),
            )?;
        state_transaction.refresh_axt_policies_from_directory();

        Ok(())
    }
}

impl Execute for RevokeSpaceDirectoryManifest {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let dataspace = self.dataspace;
        ensure_known_dataspace(state_transaction, dataspace)?;
        if !has_publish_permission(state_transaction, authority, dataspace) {
            return Err(InstructionExecutionError::InvariantViolation(
                "not permitted: CanPublishSpaceDirectoryManifest".into(),
            ));
        }

        let uaid = self.uaid;
        let mut set = state_transaction
            .world
            .space_directory_manifests
            .get(&uaid)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "Space Directory manifest does not exist for UAID".into(),
                )
            })?;
        let mut record = set.get(&dataspace).cloned().ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "Space Directory manifest does not exist for dataspace".into(),
            )
        })?;

        record
            .lifecycle
            .mark_revoked(self.revoked_epoch, self.reason.clone());
        let manifest_hash = record.manifest_hash;
        set.upsert(record);
        state_transaction
            .world
            .space_directory_manifests
            .insert(uaid, set);
        state_transaction.rebuild_space_directory_bindings(uaid);
        state_transaction.refresh_axt_policies_from_directory();

        state_transaction
            .world
            .emit_events(Some(SpaceDirectoryEvent::ManifestRevoked(
                SpaceDirectoryManifestRevoked {
                    dataspace,
                    uaid,
                    manifest_hash,
                    revoked_epoch: self.revoked_epoch,
                    reason: self.reason.clone(),
                },
            )));
        #[cfg(feature = "telemetry")]
        {
            state_transaction
                .telemetry
                .record_space_directory_revision(dataspace);
        }

        Ok(())
    }
}

fn upsert_manifest(
    state_transaction: &mut StateTransaction<'_, '_>,
    uaid: UniversalAccountId,
    record: SpaceDirectoryManifestRecord,
) {
    let mut set = state_transaction
        .world
        .space_directory_manifests
        .get(&uaid)
        .cloned()
        .unwrap_or_default();
    set.upsert(record);
    state_transaction
        .world
        .space_directory_manifests
        .insert(uaid, set);
}

fn ensure_known_dataspace(
    state_transaction: &StateTransaction<'_, '_>,
    dataspace: DataSpaceId,
) -> Result<(), Error> {
    if state_transaction
        .nexus
        .dataspace_catalog
        .entries()
        .iter()
        .any(|entry| entry.id == dataspace)
    {
        return Ok(());
    }

    Err(
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(format!(
            "unknown dataspace id {}",
            dataspace.as_u64()
        )))
        .into(),
    )
}

fn has_publish_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    dataspace: DataSpaceId,
) -> bool {
    if has_permission_in_source(
        state_transaction.world.account_permissions.get(authority),
        dataspace,
    ) {
        return true;
    }

    let role_ids: Vec<_> = state_transaction
        .world
        .account_roles_iter(authority)
        .cloned()
        .collect();
    for role_id in role_ids {
        if let Some(role) = state_transaction.world.roles.get(&role_id) {
            if permissions_allow_manifest(role.permissions(), dataspace) {
                return true;
            }
        }
    }
    false
}

const MANIFEST_PERMISSION: &str = "CanPublishSpaceDirectoryManifest";

fn has_permission_in_source(permissions: Option<&Permissions>, dataspace: DataSpaceId) -> bool {
    permissions.is_some_and(|perms| permissions_allow_manifest(perms, dataspace))
}

#[allow(single_use_lifetimes)]
fn permissions_allow_manifest<'a>(
    permissions: impl IntoIterator<Item = &'a Permission>,
    dataspace: DataSpaceId,
) -> bool {
    permissions.into_iter().any(|permission| {
        permission.name() == MANIFEST_PERMISSION
            && permission
                .payload()
                .try_into_any_norito::<CanPublishSpaceDirectoryManifest>()
                .is_ok_and(|token| token.dataspace == dataspace)
    })
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, KeyPair};
    use iroha_data_model::isi::error::InvalidParameterError;
    use iroha_data_model::{
        account::NewAccount,
        block::BlockHeader,
        domain::{Domain, DomainId},
        events::{
            EventBox,
            data::{DataEvent, space_directory::SpaceDirectoryEvent},
        },
        metadata::Metadata,
        nexus::{AssetPermissionManifest, DataSpaceCatalog, DataSpaceMetadata, ManifestVersion},
        permission::Permissions,
        prelude::Register,
    };
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        nexus::space_directory::SpaceDirectoryManifestSet,
        state::{State, World},
    };

    fn test_state() -> State {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    fn grant_manifest_permission(world: &mut World, authority: &AccountId, dataspace: DataSpaceId) {
        let mut permissions = Permissions::new();
        permissions.insert(Permission::from(CanPublishSpaceDirectoryManifest {
            dataspace,
        }));
        world
            .account_permissions
            .insert(authority.clone(), permissions);
    }

    fn sample_manifest(
        uaid: UniversalAccountId,
        dataspace: DataSpaceId,
        issued_ms: u64,
    ) -> AssetPermissionManifest {
        AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid,
            dataspace,
            issued_ms,
            activation_epoch: 5,
            expiry_epoch: None,
            entries: Vec::new(),
        }
    }

    fn seed_domain(state: &mut State, id: &DomainId, owner: &AccountId) {
        let domain = Domain {
            id: id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: owner.clone(),
        };
        state.world.domains.insert(id.clone(), domain);
    }

    fn seed_dataspace_catalog(state: &mut State, dataspace: DataSpaceId) {
        let mut entries = state.nexus.read().dataspace_catalog.entries().to_vec();
        if entries.iter().all(|entry| entry.id != dataspace) {
            entries.push(DataSpaceMetadata {
                id: dataspace,
                alias: format!("dataspace_{}", dataspace.as_u64()),
                description: None,
                fault_tolerance: 1,
            });
        }
        state.nexus.write().dataspace_catalog =
            DataSpaceCatalog::new(entries).expect("dataspace catalog");
    }

    #[test]
    fn publish_manifest_requires_permission() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::perm"));
        let dataspace = DataSpaceId::new(11);
        seed_dataspace_catalog(&mut state, dataspace);
        let manifest = sample_manifest(uaid, dataspace, 1);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let err = PublishSpaceDirectoryManifest { manifest }
            .execute(&authority, &mut tx)
            .expect_err("permission missing");
        let message = err.to_string();
        assert!(
            message.contains("CanPublishSpaceDirectoryManifest"),
            "error references missing permission: {message}"
        );
    }

    #[test]
    fn publish_manifest_records_manifest_snapshot() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::publish"));
        let dataspace = DataSpaceId::new(42);
        seed_dataspace_catalog(&mut state, dataspace);
        grant_manifest_permission(&mut state.world, &authority, dataspace);
        let manifest = sample_manifest(uaid, dataspace, 5);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        PublishSpaceDirectoryManifest {
            manifest: manifest.clone(),
        }
        .execute(&authority, &mut tx)
        .expect("publish manifest");
        tx.apply();
        block.commit().unwrap();

        let view = state.view();
        let stored = view
            .world()
            .space_directory_manifests()
            .get(&uaid)
            .and_then(|set| set.get(&dataspace))
            .expect("manifest stored");
        assert_eq!(stored.manifest.uaid, uaid);
        assert_eq!(stored.manifest.dataspace, dataspace);
        assert_eq!(stored.manifest.issued_ms, manifest.issued_ms);
        assert_eq!(
            stored.lifecycle.activated_epoch,
            Some(manifest.activation_epoch),
            "publish marks manifest active"
        );
        assert!(
            view.world().uaid_dataspaces().get(&uaid).is_none(),
            "no UAID accounts were registered, so bindings remain empty"
        );
    }

    #[test]
    fn publishing_replaces_existing_manifest_and_rebuilds_bindings() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let dataspace = DataSpaceId::new(7);
        seed_dataspace_catalog(&mut state, dataspace);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::rotate"));
        grant_manifest_permission(&mut state.world, &authority, dataspace);

        let domain_id: DomainId = "space.publish".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account =
            NewAccount::new_in_domain(account_id.clone(), domain_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account with UAID");

        let mut active_manifest =
            SpaceDirectoryManifestRecord::new(sample_manifest(uaid, dataspace, 10));
        active_manifest.lifecycle.mark_activated(3);
        let mut set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
        set.upsert(active_manifest);
        tx.world.space_directory_manifests.insert(uaid, set);
        tx.rebuild_space_directory_bindings(uaid);
        assert!(
            tx.world
                .uaid_dataspaces
                .get(&uaid)
                .is_some_and(|bindings| !bindings.is_empty()),
            "active manifest binds account"
        );

        PublishSpaceDirectoryManifest {
            manifest: sample_manifest(uaid, dataspace, 20),
        }
        .execute(&authority, &mut tx)
        .expect("replace manifest");
        tx.apply();
        block.commit().unwrap();

        let view = state.view();
        let set = view
            .world()
            .space_directory_manifests()
            .get(&uaid)
            .expect("manifest registry exists");
        let record = set
            .get(&dataspace)
            .expect("dataspace entry after replacement");
        assert_eq!(record.manifest.issued_ms, 20);
        let bindings = view
            .world()
            .uaid_dataspaces()
            .get(&uaid)
            .expect("bindings remain after replacement");
        assert!(
            bindings
                .iter()
                .any(|(id, accounts)| *id == dataspace && accounts.contains(&account_id)),
            "replacement keeps UAID bound to dataspace"
        );
    }

    #[test]
    fn publish_manifest_emits_activation_event_and_binds_accounts() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let dataspace = DataSpaceId::new(17);
        seed_dataspace_catalog(&mut state, dataspace);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::activate"));
        grant_manifest_permission(&mut state.world, &authority, dataspace);

        let domain_id: DomainId = "spaces.activate".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let keypair = KeyPair::random();
        let account_id = AccountId::new(keypair.public_key().clone());
        let new_account =
            NewAccount::new_in_domain(account_id.clone(), domain_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account");
        tx.world.take_external_events();

        PublishSpaceDirectoryManifest {
            manifest: sample_manifest(uaid, dataspace, 30),
        }
        .execute(&authority, &mut tx)
        .expect("publish manifest");

        let bindings = tx
            .world
            .uaid_dataspaces
            .get(&uaid)
            .expect("bindings created on activation");
        assert!(
            bindings
                .iter()
                .any(|(id, accounts)| *id == dataspace && accounts.contains(&account_id)),
            "account bound to dataspace after activation"
        );

        let events = tx.world.take_external_events();
        let activated = events
            .into_iter()
            .find_map(|event| match event {
                EventBox::Data(shared) => match shared.as_ref() {
                    DataEvent::SpaceDirectory(space_event) => Some(space_event.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("activation event emitted");
        match activated {
            SpaceDirectoryEvent::ManifestActivated(payload) => {
                assert_eq!(payload.dataspace, dataspace);
                assert_eq!(payload.uaid, uaid);
                assert_eq!(payload.activation_epoch, 5);
                assert_eq!(payload.expiry_epoch, None);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn revoke_manifest_marks_lifecycle_and_emits_event() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::revoke"));
        let dataspace = DataSpaceId::new(55);
        seed_dataspace_catalog(&mut state, dataspace);
        grant_manifest_permission(&mut state.world, &authority, dataspace);

        let domain_id: DomainId = "spaces.revoke".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let kp = KeyPair::random();
        let account_id = AccountId::new(kp.public_key().clone());
        let new_account =
            NewAccount::new_in_domain(account_id, domain_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account");

        let mut set = SpaceDirectoryManifestSet::default();
        let mut record = SpaceDirectoryManifestRecord::new(sample_manifest(uaid, dataspace, 5));
        record.lifecycle.mark_activated(3);
        set.upsert(record);
        tx.world.space_directory_manifests.insert(uaid, set);
        tx.rebuild_space_directory_bindings(uaid);
        assert!(
            tx.world
                .uaid_dataspaces
                .get(&uaid)
                .is_some_and(|bindings| !bindings.is_empty()),
            "bindings exist prior to revocation"
        );
        tx.world.take_external_events();

        RevokeSpaceDirectoryManifest {
            uaid,
            dataspace,
            revoked_epoch: 12,
            reason: Some("policy review".to_string()),
        }
        .execute(&authority, &mut tx)
        .expect("revoke manifest");

        let set = tx
            .world
            .space_directory_manifests
            .get(&uaid)
            .cloned()
            .expect("manifest set present");
        let record = set.get(&dataspace).expect("record exists");
        assert_eq!(record.lifecycle.revocation.as_ref().unwrap().epoch, 12);
        assert_eq!(
            record
                .lifecycle
                .revocation
                .as_ref()
                .unwrap()
                .reason
                .as_deref(),
            Some("policy review")
        );
        assert!(
            tx.world.uaid_dataspaces.get(&uaid).is_none(),
            "bindings cleared by revocation"
        );

        let events = tx.world.take_external_events();
        let revoked = events
            .into_iter()
            .find_map(|event| match event {
                EventBox::Data(shared) => match shared.as_ref() {
                    DataEvent::SpaceDirectory(space_event) => Some(space_event.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("revocation event emitted");
        match revoked {
            SpaceDirectoryEvent::ManifestRevoked(payload) => {
                assert_eq!(payload.dataspace, dataspace);
                assert_eq!(payload.uaid, uaid);
                assert_eq!(payload.revoked_epoch, 12);
                assert_eq!(payload.reason.as_deref(), Some("policy review"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn expire_manifest_marks_lifecycle_and_emits_event() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::expire"));
        let dataspace = DataSpaceId::new(88);
        seed_dataspace_catalog(&mut state, dataspace);
        grant_manifest_permission(&mut state.world, &authority, dataspace);

        let domain_id: DomainId = "spaces.expire".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);
        let kp = KeyPair::random();
        let account_id = AccountId::new(kp.public_key().clone());
        let new_account =
            NewAccount::new_in_domain(account_id, domain_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account");

        let mut set = SpaceDirectoryManifestSet::default();
        let mut record = SpaceDirectoryManifestRecord::new(sample_manifest(uaid, dataspace, 9));
        record.lifecycle.mark_activated(4);
        set.upsert(record);
        tx.world.space_directory_manifests.insert(uaid, set);
        tx.world.rebuild_space_directory_bindings(uaid);
        assert!(
            tx.world
                .uaid_dataspaces
                .get(&uaid)
                .is_some_and(|bindings| !bindings.is_empty()),
            "bindings exist prior to expiry"
        );
        tx.world.take_external_events();

        ExpireSpaceDirectoryManifest {
            uaid,
            dataspace,
            expired_epoch: 99,
        }
        .execute(&authority, &mut tx)
        .expect("expire manifest");

        let set = tx
            .world
            .space_directory_manifests
            .get(&uaid)
            .cloned()
            .expect("manifest set present");
        let record = set.get(&dataspace).expect("record exists");
        assert_eq!(record.lifecycle.expired_epoch, Some(99));
        assert!(
            record.lifecycle.revocation.is_none(),
            "expiry should not mark revocation"
        );
        assert!(
            tx.world.uaid_dataspaces.get(&uaid).is_none(),
            "bindings cleared by expiry"
        );

        let events = tx.world.take_external_events();
        let expired = events
            .into_iter()
            .find_map(|event| match event {
                EventBox::Data(shared) => match shared.as_ref() {
                    DataEvent::SpaceDirectory(space_event) => Some(space_event.clone()),
                    _ => None,
                },
                _ => None,
            })
            .expect("expiry event emitted");
        match expired {
            SpaceDirectoryEvent::ManifestExpired(payload) => {
                assert_eq!(payload.dataspace, dataspace);
                assert_eq!(payload.uaid, uaid);
                assert_eq!(payload.expired_epoch, 99);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn publish_manifest_rejects_unknown_dataspace() {
        let mut state = test_state();
        state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::GLOBAL,
            alias: "global".to_string(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("dataspace catalog");

        let authority = (*ALICE_ID).clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::unknown-publish"));
        let dataspace = DataSpaceId::new(404);
        grant_manifest_permission(&mut state.world, &authority, dataspace);
        let manifest = sample_manifest(uaid, dataspace, 1);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let err = PublishSpaceDirectoryManifest { manifest }
            .execute(&authority, &mut tx)
            .expect_err("unknown dataspace should be rejected");
        let Error::InvalidParameter(InvalidParameterError::SmartContract(message)) = err else {
            panic!("unexpected error: {err:?}");
        };
        assert!(
            message.contains("unknown dataspace id"),
            "error should mention unknown dataspace: {message}"
        );
    }

    #[test]
    fn revoke_manifest_rejects_unknown_dataspace() {
        let mut state = test_state();
        state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::GLOBAL,
            alias: "global".to_string(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("dataspace catalog");

        let authority = (*ALICE_ID).clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::unknown-revoke"));
        let dataspace = DataSpaceId::new(405);
        grant_manifest_permission(&mut state.world, &authority, dataspace);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let err = RevokeSpaceDirectoryManifest {
            uaid,
            dataspace,
            revoked_epoch: 5,
            reason: None,
        }
        .execute(&authority, &mut tx)
        .expect_err("unknown dataspace should be rejected");
        let Error::InvalidParameter(InvalidParameterError::SmartContract(message)) = err else {
            panic!("unexpected error: {err:?}");
        };
        assert!(
            message.contains("unknown dataspace id"),
            "error should mention unknown dataspace: {message}"
        );
    }

    #[test]
    fn expire_manifest_rejects_unknown_dataspace() {
        let mut state = test_state();
        state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::GLOBAL,
            alias: "global".to_string(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("dataspace catalog");

        let authority = (*ALICE_ID).clone();
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::unknown-expire"));
        let dataspace = DataSpaceId::new(406);
        grant_manifest_permission(&mut state.world, &authority, dataspace);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        let err = ExpireSpaceDirectoryManifest {
            uaid,
            dataspace,
            expired_epoch: 7,
        }
        .execute(&authority, &mut tx)
        .expect_err("unknown dataspace should be rejected");
        let Error::InvalidParameter(InvalidParameterError::SmartContract(message)) = err else {
            panic!("unexpected error: {err:?}");
        };
        assert!(
            message.contains("unknown dataspace id"),
            "error should mention unknown dataspace: {message}"
        );
    }
}
