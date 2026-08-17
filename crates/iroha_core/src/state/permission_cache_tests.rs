use super::*;
use crate::{
    prelude::{AcceptedTransaction, StateReadOnly},
    smartcontracts::Execute,
};
use iroha_data_model::{
    account::AccountId,
    domain::DomainId,
    isi::{Grant, Revoke},
    nexus::DataSpaceId,
    permission::Permission,
    prelude::{Account, Domain},
    role::{Role, RoleId},
    trigger::TriggerId,
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias},
    role::CanManageRoles,
    trigger::{CanExecuteTrigger, CanRegisterTrigger},
};
use iroha_primitives::json::Json;
use iroha_test_samples::gen_account_in;
use nonzero_ext::nonzero;
use std::collections::BTreeSet;
fn wonderland_domain_id() -> DomainId {
    DomainId::try_new("wonderland", "universal").expect("domain id")
}
fn new_wonderland_account(account_id: &AccountId) -> iroha_data_model::account::NewAccount {
    Account::new(account_id.clone())
}
fn new_genesis_account(account_id: &AccountId) -> iroha_data_model::account::NewAccount {
    Account::new(account_id.clone())
}
#[test]
fn revoke_permission_invalidates_trigger_cache() {
    let (registrar, _) = gen_account_in("wonderland");
    let (owner, _) = gen_account_in("wonderland");
    let domain: Domain = Domain::new(wonderland_domain_id()).build(&registrar);
    let registrar_account = new_wonderland_account(&registrar).build(&registrar);
    let owner_account = new_wonderland_account(&owner).build(&registrar);
    let world = World::with([domain], [registrar_account, owner_account], []);
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(world, kura, query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let permission = CanRegisterTrigger {
        authority: owner.clone(),
    };
    Grant::account_permission(permission.clone(), registrar.clone())
        .execute(&registrar, &mut stx)
        .expect("grant trigger permission");
    assert!(
        stx.can_register_trigger_for(&registrar, &owner),
        "permission should allow trigger registration"
    );
    Revoke::account_permission(permission, registrar.clone())
        .execute(&registrar, &mut stx)
        .expect("revoke trigger permission");
    assert!(
        !stx.can_register_trigger_for(&registrar, &owner),
        "cache must be invalidated after revoke"
    );
}
#[test]
fn trigger_permission_payload_with_whitespace_decodes() {
    let (registrar, _) = gen_account_in("wonderland");
    let domain: Domain = Domain::new(wonderland_domain_id()).build(&registrar);
    let registrar_account = new_wonderland_account(&registrar).build(&registrar);
    let world = World::with([domain], [registrar_account], []);
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(world, kura, query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let trigger_id: TriggerId = "trigger_alpha".parse().unwrap();
    let raw_payload = "{  \"trigger\"  :   \"trigger_alpha\" }";
    let permission = iroha_data_model::permission::Permission::new(
        "CanExecuteTrigger".into(),
        Json::from_raw_json(raw_payload.to_owned()).expect("valid permission JSON fixture"),
    );
    stx.world
        .account_permissions
        .insert(registrar.clone(), BTreeSet::from([permission]));
    assert!(
        stx.can_execute_trigger_for(&registrar, &trigger_id),
        "Norito decoder should handle non-canonical JSON payloads"
    );
    assert!(stx.can_execute_trigger_for(&registrar, &trigger_id));
}
#[test]
fn permission_deserialized_from_json_matches_canonical_permission() {
    let stored: Permission = norito::json::from_str(
        r#"{
            "name": "CanManageAccountAlias",
            "payload": { "scope": { "scope": "dataspace", "value": 0 } }
        }"#,
    )
    .expect("deserialize permission");
    let target = Permission::from(CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
    });
    let permissions = BTreeSet::from([stored]);
    assert!(
        permissions.contains(&target),
        "deserialized permissions should use canonical JSON payloads: stored={}, target={}",
        permissions
            .first()
            .expect("stored permission")
            .payload()
            .get(),
        target.payload().get(),
    );
}
#[test]
fn role_granted_trigger_permissions_cache_and_invalidate() {
    let (registrar, _) = gen_account_in("wonderland");
    let (owner, _) = gen_account_in("wonderland");
    let domain: Domain = Domain::new(wonderland_domain_id()).build(&registrar);
    let registrar_account = new_wonderland_account(&registrar).build(&registrar);
    let owner_account = new_wonderland_account(&owner).build(&registrar);
    let role_id: RoleId = "trigger_role".parse().unwrap();
    let trigger_id: TriggerId = "trigger_alpha".parse().unwrap();
    let role = Role::new(role_id.clone(), registrar.clone())
        .add_permission(CanRegisterTrigger {
            authority: owner.clone(),
        })
        .add_permission(CanExecuteTrigger {
            trigger: trigger_id.clone(),
        })
        .build(&registrar);
    let mut world = World::with([domain], [registrar_account, owner_account], []);
    assert!(world.roles.insert(role_id.clone(), role).is_none());
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(world, kura, query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    assert!(
        !stx.can_register_trigger_for(&registrar, &owner),
        "role permissions should not apply before membership"
    );
    assert!(
        !stx.can_execute_trigger_for(&registrar, &trigger_id),
        "role permissions should not apply before membership"
    );
    Grant::account_role(role_id.clone(), registrar.clone())
        .execute(&registrar, &mut stx)
        .expect("grant account role");
    assert!(
        stx.can_register_trigger_for(&registrar, &owner),
        "granting role should allow trigger registration"
    );
    assert!(
        stx.can_execute_trigger_for(&registrar, &trigger_id),
        "granting role should allow trigger execution"
    );
    // Cached value should remain true while role membership stays in place.
    assert!(stx.can_register_trigger_for(&registrar, &owner));
    assert!(stx.can_execute_trigger_for(&registrar, &trigger_id));
    Revoke::account_role(role_id, registrar.clone())
        .execute(&registrar, &mut stx)
        .expect("revoke account role");
    assert!(
        !stx.can_register_trigger_for(&registrar, &owner),
        "revoking role should invalidate cache and revoke registration permission"
    );
    assert!(
        !stx.can_execute_trigger_for(&registrar, &trigger_id),
        "revoking role should invalidate cache and revoke execution permission"
    );
}
fn previous_roster_evidence_for_parent(
    parent: &SignedBlock,
    roster: &[PeerId],
) -> iroha_data_model::consensus::PreviousRosterEvidence {
    let zero_state_root = iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]);
    let mut signers_bitmap = vec![0_u8; roster.len().div_ceil(8)];
    if let Some(first_byte) = signers_bitmap.first_mut() {
        *first_byte = 1;
    }
    iroha_data_model::consensus::PreviousRosterEvidence {
        height: parent.header().height().get(),
        block_hash: parent.hash(),
        validator_checkpoint: iroha_data_model::consensus::ValidatorSetCheckpoint::new(
            parent.header().height().get(),
            parent.header().view_change_index(),
            parent.hash(),
            zero_state_root,
            zero_state_root,
            roster.to_vec(),
            signers_bitmap,
            Vec::new(),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            None,
        ),
        stake_snapshot: None,
    }
}
fn build_test_block(
    accepted: AcceptedTransaction<'static>,
    parent: Option<&SignedBlock>,
    topology: &crate::sumeragi::network_topology::Topology,
    signer: &iroha_crypto::PrivateKey,
) -> crate::block::NewBlock {
    let mut builder = crate::block::BlockBuilder::new(vec![accepted]).chain(0, parent);
    if let Some(parent) = parent.filter(|block| block.header().height().get() >= 2) {
        builder = builder.with_previous_roster_evidence(Some(previous_roster_evidence_for_parent(
            parent,
            topology.as_ref(),
        )));
    }
    builder.sign(signer).unpack(|_| {})
}
fn install_permission_cache_replay_parameters(state: &State) {
    let mut parameters = state.world.parameters.block();
    parameters.set_parameter(iroha_data_model::parameter::system::Parameter::Custom(
        iroha_data_model::parameter::system::SumeragiNposParameters::default()
            .into_custom_parameter(),
    ));
    parameters.commit();
}
fn replay_permission_cache_blocks(
    kura: &Arc<Kura>,
    state: &mut State,
    topology: &crate::sumeragi::network_topology::Topology,
    block_count: usize,
) -> Result<()> {
    super::replay_validation_tests::replay_blocks_from_kura_range(
        kura,
        state,
        topology,
        1,
        1,
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
    )?;
    install_permission_cache_replay_parameters(state);
    if block_count > 1 {
        super::replay_validation_tests::replay_blocks_from_kura_range(
            kura,
            state,
            topology,
            2,
            block_count,
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
        )?;
    }
    Ok(())
}
#[test]
fn permission_cache_rebuilds_after_restart() {
    // The full replay pipeline has deep debug-mode stack use; do not depend on libtest's
    // platform-default worker stack for this integration-heavy scenario.
    let handle = std::thread::Builder::new()
        .name("permission_cache_rebuilds_after_restart".to_owned())
        .stack_size(16 * 1024 * 1024)
        .spawn(permission_cache_rebuilds_after_restart_impl)
        .expect("spawn permission cache replay test");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}
#[allow(clippy::too_many_lines)]
fn permission_cache_rebuilds_after_restart_impl() {
    use iroha_config::{
        base::WithOrigin,
        kura::InitMode,
        parameters::{
            actual::{Kura as Config, LaneConfig},
            defaults::kura::BLOCKS_IN_MEMORY,
        },
    };
    use iroha_data_model::{
        ChainId,
        block::{BlockHeader, SignedBlock},
        domain::Domain,
        isi::{Grant, InstructionBox},
        prelude::PeerId,
        transaction::TransactionBuilder,
        trigger::TriggerId,
    };
    use iroha_genesis::{GENESIS_DOMAIN_ID, GenesisBuilder, GenesisTopologyEntry};
    use iroha_primitives::time::TimeSource;
    use iroha_test_samples::{
        SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR, gen_account_in,
    };
    use std::{
        borrow::Cow,
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    #[cfg(debug_assertions)]
    {
        println!(
            "permission_cache_rebuilds_after_restart temp dir: {}",
            temp_dir.path().display()
        );
    }
    let make_config = |dir: &tempfile::TempDir| Config {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(dir.path().to_path_buf()),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity:
            iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: iroha_config::kura::FsyncMode::Batched,
        fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention:
            iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention:
            iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
    };
    let lane_config = LaneConfig::default();
    let (kura, _) = Kura::new(&make_config(&temp_dir), &lane_config).expect("init kura");
    let live_query = {
        let _guard = runtime.enter();
        crate::query::store::LiveQueryStore::start_test()
    };
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let make_world = || {
        World::with(
            [Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_id)],
            [new_genesis_account(&genesis_id).build(&genesis_id)],
            [],
        )
    };
    let state = State::new(make_world(), Arc::clone(&kura), live_query);
    {
        let mut params_block = state.world.parameters.block();
        params_block.sumeragi.key_require_hsm = false;
        params_block.commit();
    }
    let mut recorded_blocks: Vec<Arc<SignedBlock>> = Vec::new();
    let leader_keypair =
        crate::state::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
    let (leader_public_key, leader_private_key) = leader_keypair.into_parts();
    let topology = crate::sumeragi::network_topology::Topology::new(vec![PeerId::new(
        leader_public_key.clone(),
    )]);
    let leader_pop =
        iroha_crypto::bls_normal_pop_prove(&leader_private_key).expect("generate BLS PoP");
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let (registrar, registrar_keypair) = gen_account_in("wonderland");
    let (owner, owner_keypair) = gen_account_in("wonderland");
    let trigger_id: TriggerId = "trigger_alpha".parse().unwrap();
    let mut genesis_builder =
        GenesisBuilder::new_without_executor(chain_id.clone(), "ivm/libs/not/installed")
            .set_topology(vec![GenesisTopologyEntry::new(
                PeerId::new(leader_public_key.clone()),
                leader_pop,
            )]);
    genesis_builder = genesis_builder
        .domain(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .account(registrar_keypair.public_key().clone())
        .account(owner_keypair.public_key().clone())
        .finish_domain()
        .append_instruction(Register::trigger(iroha_data_model::trigger::Trigger::new(
            trigger_id.clone(),
            iroha_data_model::trigger::action::Action::new(
                vec![InstructionBox::from(Log::new(
                    iroha_logger::Level::INFO,
                    "permission cache trigger".to_owned(),
                ))],
                iroha_data_model::trigger::action::Repeats::Indefinitely,
                owner.clone(),
                iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter::new()
                    .for_trigger(trigger_id.clone())
                    .under_authority(owner.clone()),
            )
            .expect("trigger action fixture satisfies validation invariants"),
        )))
        .append_instruction(Grant::account_permission(CanManageRoles, owner.clone()));
    let genesis_block = genesis_builder
        .build_and_sign(&SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
        .expect("genesis");
    {
        let time_source = TimeSource::new_system();
        let mut voting_block = None;
        let (valid_genesis, mut state_block) =
            crate::block::ValidBlock::validate_signed_genesis_keep_voting_block(
                genesis_block.0.clone(),
                &topology,
                &genesis_id,
                &time_source,
                &state,
                &mut voting_block,
                iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            )
            .unpack(|_| {})
            .expect("valid genesis");
        let committed_genesis = valid_genesis.commit_unchecked().unpack(|_| {});
        let _ =
            state_block.apply_without_execution(&committed_genesis, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        let block_arc = Arc::new(committed_genesis.into());
        kura.store_block(Arc::clone(&block_arc))
            .expect("store genesis block");
        let height = block_arc.header().height().get();
        kura.store_wsv_checkpoint(
            height,
            block_arc.hash(),
            crate::snapshot::canonical_state_snapshot_hash(&state),
        )
        .expect("store genesis WSV checkpoint");
        let height_usize =
            usize::try_from(height).expect("block height must fit in usize for tests");
        assert!(
            kura.get_block(NonZeroUsize::new(height_usize).expect("height fits"))
                .is_some(),
            "genesis block should persist"
        );
        recorded_blocks.push(Arc::clone(&block_arc));
    }
    install_permission_cache_replay_parameters(&state);
    {
        let state_view = state.view();
        let world_view = state_view.world();
        assert!(
            world_view.accounts().get(&registrar).is_some(),
            "registrar account should exist after genesis"
        );
        assert!(
            world_view.accounts().get(&owner).is_some(),
            "owner account should exist after genesis"
        );
    }
    let permission_register = CanRegisterTrigger {
        authority: owner.clone(),
    };
    let permission_execute = CanExecuteTrigger {
        trigger: trigger_id.clone(),
    };
    {
        let latest_hash = state
            .view()
            .latest_block()
            .as_ref()
            .map(|block| block.hash());
        let next_height = NonZeroU64::new(2).expect("non-zero height");
        let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        Grant::account_permission(permission_register.clone(), registrar.clone())
            .execute(&owner, &mut stx)
            .expect("dry-run grant register");
        Grant::account_permission(permission_execute.clone(), registrar.clone())
            .execute(&owner, &mut stx)
            .expect("dry-run grant execute");
    }
    let grant_tx = TransactionBuilder::new(
        state.network_id,
        owner.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Grant::account_permission(
            permission_register.clone(),
            registrar.clone(),
        )),
        InstructionBox::from(Grant::account_permission(
            permission_execute.clone(),
            registrar.clone(),
        )),
    ])
    .sign(owner_keypair.private_key());
    let accepted_grant = AcceptedTransaction::new_unchecked(Cow::Owned(grant_tx));
    let latest_block = state.view().latest_block();
    let unverified_grant = build_test_block(
        accepted_grant,
        latest_block.as_deref(),
        &topology,
        &leader_private_key,
    );
    {
        let mut state_block = state.block(unverified_grant.header());
        let committed_grant = unverified_grant
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit_unchecked()
            .unpack(|_| {});
        let _ = state_block.apply_without_execution(&committed_grant, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        let signed_block: SignedBlock = committed_grant.into();
        assert!(
            signed_block.error(0).is_none(),
            "grant transaction rejected: {:?}",
            signed_block.error(0)
        );
        let block_arc = Arc::new(signed_block);
        kura.store_block(Arc::clone(&block_arc))
            .expect("store grant block");
        let height = block_arc.header().height().get();
        kura.store_wsv_checkpoint(
            height,
            block_arc.hash(),
            crate::snapshot::canonical_state_snapshot_hash(&state),
        )
        .expect("store grant WSV checkpoint");
        let height_usize =
            usize::try_from(height).expect("block height must fit in usize for tests");
        assert!(
            kura.get_block(NonZeroUsize::new(height_usize).expect("height fits"))
                .is_some(),
            "grant block should persist"
        );
        recorded_blocks.push(Arc::clone(&block_arc));
    }
    {
        let state_view = state.view();
        let world_view = state_view.world();
        assert!(
            world_view.account_permissions().get(&registrar).is_some(),
            "grant block should register permissions"
        );
    }
    {
        let latest_hash = state
            .view()
            .latest_block()
            .as_ref()
            .map(|block| block.hash());
        let next_height = NonZeroU64::new(3).expect("non-zero height");
        let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        let summary = stx.ensure_permission_summary(&registrar);
        let reg_cached = summary.reg_trigger_authorities.len();
        let exec_cached = summary.exec_trigger_ids.len();
        assert!(
            stx.can_register_trigger_for(&registrar, &owner),
            "permission should exist before restart (cached reg entries: {reg_cached})"
        );
        assert!(
            stx.can_execute_trigger_for(&registrar, &trigger_id),
            "execute permission should exist before restart (cached exec entries: {exec_cached})"
        );
    }
    drop(state);
    let live_query = {
        let _guard = runtime.enter();
        crate::query::store::LiveQueryStore::start_test()
    };
    let mut state = State::new(make_world(), Arc::clone(&kura), live_query);
    {
        let mut params_block = state.world.parameters.block();
        params_block.sumeragi.key_require_hsm = false;
        params_block.commit();
    }
    replay_permission_cache_blocks(&kura, &mut state, &topology, recorded_blocks.len())
        .expect("replay stored blocks");
    {
        let latest_hash = state
            .view()
            .latest_block()
            .as_ref()
            .map(|block| block.hash());
        let next_height =
            NonZeroU64::new((recorded_blocks.len() + 1) as u64).expect("non-zero height");
        let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        let mut summary = AccountPermissionSummary::default();
        summary.apply_grant(
            &stx.world,
            &stx.nexus.dataspace_catalog,
            &registrar,
            &Permission::from(permission_register.clone()),
            stx.block_unix_timestamp_ms(),
        );
        summary.apply_grant(
            &stx.world,
            &stx.nexus.dataspace_catalog,
            &registrar,
            &Permission::from(permission_execute.clone()),
            stx.block_unix_timestamp_ms(),
        );
        stx.perm_cache.insert_summary(registrar.clone(), summary);
        assert!(
            stx.can_register_trigger_for(&registrar, &owner),
            "permission should exist after replay"
        );
        assert!(
            stx.can_execute_trigger_for(&registrar, &trigger_id),
            "execute permission should exist after replay"
        );
        let summary = stx.ensure_permission_summary(&registrar);
        let reg_cached = summary.reg_trigger_authorities.len();
        let exec_cached = summary.exec_trigger_ids.len();
        assert_eq!(reg_cached, 1);
        assert_eq!(exec_cached, 1);
        assert!(
            stx.can_register_trigger_for(&registrar, &owner),
            "repeat hit should stay cached"
        );
        assert_eq!(
            stx.ensure_permission_summary(&registrar)
                .reg_trigger_authorities
                .len(),
            reg_cached
        );
    }
    let revoke_tx = TransactionBuilder::new(
        state.network_id,
        owner.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Revoke::account_permission(
            permission_register.clone(),
            registrar.clone(),
        )),
        InstructionBox::from(Revoke::account_permission(
            permission_execute.clone(),
            registrar.clone(),
        )),
    ])
    .sign(owner_keypair.private_key());
    let accepted_revoke = AcceptedTransaction::new_unchecked(Cow::Owned(revoke_tx));
    let latest_block = state.view().latest_block();
    let unverified_revoke = build_test_block(
        accepted_revoke,
        latest_block.as_deref(),
        &topology,
        &leader_private_key,
    );
    {
        let mut state_block = state.block(unverified_revoke.header());
        let committed_revoke = unverified_revoke
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit_unchecked()
            .unpack(|_| {});
        let _ =
            state_block.apply_without_execution(&committed_revoke, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        let block_arc = Arc::new(committed_revoke.into());
        kura.store_block(Arc::clone(&block_arc))
            .expect("store revoke block");
        let height = block_arc.header().height().get();
        kura.store_wsv_checkpoint(
            height,
            block_arc.hash(),
            crate::snapshot::canonical_state_snapshot_hash(&state),
        )
        .expect("store revoke WSV checkpoint");
        let height_usize =
            usize::try_from(height).expect("block height must fit in usize for tests");
        assert!(
            kura.get_block(NonZeroUsize::new(height_usize).expect("height fits"))
                .is_some(),
            "revoke block should persist"
        );
        recorded_blocks.push(Arc::clone(&block_arc));
    }
    let latest_hash = state
        .view()
        .latest_block()
        .as_ref()
        .map(|block| block.hash());
    let next_height = NonZeroU64::new((recorded_blocks.len() + 1) as u64).expect("non-zero height");
    let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
    {
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        assert!(
            stx.perm_cache.needs_hydration(&registrar),
            "cache should be invalidated after revoke"
        );
        assert!(
            !stx.can_register_trigger_for(&registrar, &owner),
            "registration permission revoked"
        );
        assert!(
            !stx.can_execute_trigger_for(&registrar, &trigger_id),
            "execution permission revoked"
        );
    }
    drop(state);
    let live_query = {
        let _guard = runtime.enter();
        crate::query::store::LiveQueryStore::start_test()
    };
    let mut state = State::new(make_world(), Arc::clone(&kura), live_query);
    {
        let mut params_block = state.world.parameters.block();
        params_block.sumeragi.key_require_hsm = false;
        params_block.commit();
    }
    replay_permission_cache_blocks(&kura, &mut state, &topology, recorded_blocks.len())
        .expect("replay stored blocks after revoke");
    {
        let state_view = state.view();
        let world_view = state_view.world();
        assert!(
            world_view.accounts().get(&registrar).is_some(),
            "registrar account should exist after replay"
        );
        assert!(
            world_view.accounts().get(&owner).is_some(),
            "owner account should exist after replay"
        );
    }
    let latest_hash = state
        .view()
        .latest_block()
        .as_ref()
        .map(|block| block.hash());
    let next_height = NonZeroU64::new((recorded_blocks.len() + 1) as u64).expect("non-zero height");
    let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
    {
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        assert!(
            !stx.can_register_trigger_for(&registrar, &owner),
            "registration should remain revoked after restart"
        );
        assert!(
            !stx.can_execute_trigger_for(&registrar, &trigger_id),
            "execution should remain revoked after restart"
        );
    }
    let role_id: RoleId = "trigger_role_restart".parse().unwrap();
    let role = Role::new(role_id.clone(), owner.clone())
        .add_permission(permission_register.clone())
        .add_permission(permission_execute.clone());
    let register_role_tx = TransactionBuilder::new(
        state.network_id,
        owner.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        InstructionBox::from(Register::role(role)),
        InstructionBox::from(Grant::account_role(role_id.clone(), registrar.clone())),
    ])
    .sign(owner_keypair.private_key());
    let accepted_role = AcceptedTransaction::new_unchecked(Cow::Owned(register_role_tx));
    let latest_block = state.view().latest_block();
    let unverified_role = build_test_block(
        accepted_role,
        latest_block.as_deref(),
        &topology,
        &leader_private_key,
    );
    {
        let mut state_block = state.block(unverified_role.header());
        let committed_role = unverified_role
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit_unchecked()
            .unpack(|_| {});
        let _ = state_block.apply_without_execution(&committed_role, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        let signed_block: SignedBlock = committed_role.into();
        assert!(
            signed_block.error(0).is_none(),
            "role registration transaction rejected: {:?}",
            signed_block.error(0)
        );
        let block_arc = Arc::new(signed_block);
        kura.store_block(Arc::clone(&block_arc))
            .expect("store role block");
        let height = block_arc.header().height().get();
        kura.store_wsv_checkpoint(
            height,
            block_arc.hash(),
            crate::snapshot::canonical_state_snapshot_hash(&state),
        )
        .expect("store role WSV checkpoint");
        let height_usize =
            usize::try_from(height).expect("block height must fit in usize for tests");
        assert!(
            kura.get_block(NonZeroUsize::new(height_usize).expect("height fits"))
                .is_some(),
            "role registration block should persist"
        );
        recorded_blocks.push(Arc::clone(&block_arc));
    }
    let latest_hash = state
        .view()
        .latest_block()
        .as_ref()
        .map(|block| block.hash());
    let next_height = NonZeroU64::new((recorded_blocks.len() + 1) as u64).expect("non-zero height");
    let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
    {
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        assert!(
            stx.can_register_trigger_for(&registrar, &owner),
            "role membership should allow trigger registration"
        );
        assert!(
            stx.can_execute_trigger_for(&registrar, &trigger_id),
            "role membership should allow trigger execution"
        );
    }
    drop(state);
    let live_query = {
        let _guard = runtime.enter();
        crate::query::store::LiveQueryStore::start_test()
    };
    let mut state = State::new(make_world(), Arc::clone(&kura), live_query);
    {
        let mut params_block = state.world.parameters.block();
        params_block.sumeragi.key_require_hsm = false;
        params_block.commit();
    }
    replay_permission_cache_blocks(&kura, &mut state, &topology, recorded_blocks.len())
        .expect("replay stored blocks after role grant");
    let latest_hash = state
        .view()
        .latest_block()
        .as_ref()
        .map(|block| block.hash());
    let next_height = NonZeroU64::new((recorded_blocks.len() + 1) as u64).expect("non-zero height");
    let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
    {
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        assert!(
            stx.can_register_trigger_for(&registrar, &owner),
            "role-based registration permission should survive restart"
        );
        assert!(
            stx.can_execute_trigger_for(&registrar, &trigger_id),
            "role-based execution permission should survive restart"
        );
    }
    let revoke_role_tx = TransactionBuilder::new(
        state.network_id,
        owner.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(Revoke::account_role(
        role_id.clone(),
        registrar.clone(),
    ))])
    .sign(owner_keypair.private_key());
    let accepted_revoke_role = AcceptedTransaction::new_unchecked(Cow::Owned(revoke_role_tx));
    let latest_block = state.view().latest_block();
    let unverified_revoke_role = build_test_block(
        accepted_revoke_role,
        latest_block.as_deref(),
        &topology,
        &leader_private_key,
    );
    {
        let mut state_block = state.block(unverified_revoke_role.header());
        let committed_revoke_role = unverified_revoke_role
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit_unchecked()
            .unpack(|_| {});
        let _ = state_block
            .apply_without_execution(&committed_revoke_role, topology.as_ref().to_owned());
        state_block.commit().unwrap();
        let signed_block: SignedBlock = committed_revoke_role.into();
        assert!(
            signed_block.error(0).is_none(),
            "role revocation transaction rejected: {:?}",
            signed_block.error(0)
        );
        let block_arc = Arc::new(signed_block);
        kura.store_block(Arc::clone(&block_arc))
            .expect("store revoke role block");
        let height = block_arc.header().height().get();
        kura.store_wsv_checkpoint(
            height,
            block_arc.hash(),
            crate::snapshot::canonical_state_snapshot_hash(&state),
        )
        .expect("store role revocation WSV checkpoint");
        let height_usize =
            usize::try_from(height).expect("block height must fit in usize for tests");
        assert!(
            kura.get_block(NonZeroUsize::new(height_usize).expect("height fits"))
                .is_some(),
            "role revocation block should persist"
        );
        recorded_blocks.push(Arc::clone(&block_arc));
    }
    let latest_hash = state
        .view()
        .latest_block()
        .as_ref()
        .map(|block| block.hash());
    let next_height = NonZeroU64::new((recorded_blocks.len() + 1) as u64).expect("non-zero height");
    let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
    {
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        assert!(
            stx.perm_cache.needs_hydration(&registrar),
            "revoking role should invalidate cached permissions"
        );
        assert!(
            !stx.can_register_trigger_for(&registrar, &owner),
            "role revocation should remove trigger registration permission"
        );
        assert!(
            !stx.can_execute_trigger_for(&registrar, &trigger_id),
            "role revocation should remove trigger execution permission"
        );
    }
    drop(state);
    let live_query = {
        let _guard = runtime.enter();
        crate::query::store::LiveQueryStore::start_test()
    };
    let mut state = State::new(make_world(), Arc::clone(&kura), live_query);
    {
        let mut params_block = state.world.parameters.block();
        params_block.sumeragi.key_require_hsm = false;
        params_block.commit();
    }
    replay_permission_cache_blocks(&kura, &mut state, &topology, recorded_blocks.len())
        .expect("replay stored blocks after role revoke");
    let latest_hash = state
        .view()
        .latest_block()
        .as_ref()
        .map(|block| block.hash());
    let next_height = NonZeroU64::new((recorded_blocks.len() + 1) as u64).expect("non-zero height");
    let next_header = BlockHeader::new(next_height, latest_hash, None, None, 0, 0);
    {
        let mut block = state.block(next_header);
        let mut stx = block.transaction();
        assert!(
            !stx.can_register_trigger_for(&registrar, &owner),
            "role revocation should persist after restart"
        );
        assert!(
            !stx.can_execute_trigger_for(&registrar, &trigger_id),
            "role revocation should persist after restart"
        );
    }
}
