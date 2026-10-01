//! Actual native typed originals and certified-cut tests; synthetic fixture state grants no runtime permission.
use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    IntoKeyValue, Registrable,
    account::{Account, AccountDetails},
    asset::Asset,
    common::Owned,
    domain::Domain,
    isi::Register,
    nexus::{DataSpaceCatalog, DataSpaceMetadata},
    sns::NameRecordV1,
};
use iroha_model_base::{domain::DomainId, metadata::Metadata};
use norito::codec::Encode as _;
use std::{cell::Cell, collections::BTreeSet};

fn owner(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}
fn account_world() -> (World, AccountAliasName, AccountId, Vec<u8>) {
    let mut world = World::new();
    let name: AccountAliasName = "retail@leumi.is2".parse().unwrap();
    let catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: DataSpaceId::new(77),
        alias: "is2".into(),
        description: None,
        fault_tolerance: 1,
    }])
    .unwrap();
    let alias = AccountAlias::from_literal(&name.canonical_text(), &catalog).unwrap();
    let account = owner(35);
    world.accounts.insert(
        account.clone(),
        Owned::new(AccountDetails::new(
            Metadata::default(),
            Some(alias.clone()),
            None,
            vec![],
        )),
    );
    world.account_aliases.insert(alias.clone(), account.clone());
    world.account_rekey_records.insert(
        alias.clone(),
        AccountRekeyRecord::new(alias.clone(), account.clone()),
    );
    let lease = NameRecordV1::new(
        crate::sns::selector_for_account_alias(&alias, &catalog).unwrap(),
        account.clone(),
        vec![],
        0,
        1,
        1_000_000,
        2_000_000,
        3_000_000,
        Metadata::default(),
    )
    .encode();
    world.smart_contract_state.insert(
        crate::sns::record_storage_key(
            &crate::sns::selector_for_account_alias(&alias, &catalog).unwrap(),
        ),
        lease.clone(),
    );
    world
        .smart_contract_state
        .insert("private/unrelated".parse().unwrap(), vec![99; 128]);
    (world, name, account, lease)
}
fn fee_world(present: bool) -> (World, FeeSponsorProgramId, AssetDefinitionId) {
    let mut world = World::new();
    let account = owner(36);
    let (id, value) = Account::new(account.clone())
        .build(&account)
        .into_key_value();
    world.accounts.insert(id, value);
    let fee = AssetDefinitionId::derive_from_components(
        DomainId::try_new("funding", "universal").unwrap(),
        "coin".parse().unwrap(),
    );
    let definition =
        AssetDefinition::numeric(fee.clone(), "Fixture fee", AssetBalancePolicy::Global, None)
            .build(&account);
    world.asset_definitions.insert(fee.clone(), definition);
    if present {
        let (id, value) = Asset::new(
            AssetId::with_scope(fee.clone(), account.clone(), AssetBalanceScope::Global),
            0_u32,
        )
        .into_key_value();
        world.assets.insert(id, value);
    }
    let program = FeeSponsorProgramId::new(account, "release".parse().unwrap());
    // An unrelated actual row contributes its key without disclosing its value.
    let other = FeeSponsorProgramId::new(owner(37), "unrelated".parse().unwrap());
    world
        .fee_sponsor_programs
        .insert(other.clone(), FeeSponsorProgram::new(other, owner(38)));
    (world, program, fee)
}
fn snapshot(world: &WorldBlock<'_>, budget: &AllocationBudget) -> CapturedSnapshot {
    capture(
        world,
        &WorldStateAccumulator::capture(world).unwrap(),
        budget,
    )
    .unwrap()
}
#[test]
fn account_originals_preserve_stored_label_rekey_and_server_derived_lease() {
    let (world, name, account, lease) = account_world();
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    block.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: DataSpaceId::new(77),
        alias: "is2".into(),
        description: None,
        fault_tolerance: 1,
    }])
    .unwrap();
    let captured = snapshot(&block, &budget);
    consume_account(
        &captured.snapshot,
        &block,
        &name,
        block.dataspace_catalog(),
        &budget,
        |_, alias, keys, selected| {
            assert_eq!(keys, &[alias]);
            let (bound, rekey, value, original) = selected.unwrap();
            assert_eq!(bound, &account);
            assert_eq!(&rekey.active_account_id, bound);
            assert_eq!(&rekey.label, alias);
            assert_eq!(value.as_ref().label(), Some(alias));
            assert_eq!(original, lease);
            Ok(())
        },
    )
    .unwrap();
}
#[test]
fn account_absence_requires_complete_certified_keys_and_present_rows_cannot_be_omitted() {
    let (world, name, _, _) = account_world();
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let mut block = world.block();
    block.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: DataSpaceId::new(77),
        alias: "is2".into(),
        description: None,
        fault_tolerance: 1,
    }])
    .unwrap();
    let captured = snapshot(&block, &budget);
    let absent: AccountAliasName = "other@leumi.is2".parse().unwrap();
    consume_account(
        &captured.snapshot,
        &block,
        &absent,
        block.dataspace_catalog(),
        &budget,
        |_, _, keys, selected| {
            assert_eq!(keys.len(), 1);
            assert!(selected.is_none());
            Ok(())
        },
    )
    .unwrap();
    let mut incomplete = captured.snapshot.clone();
    incomplete
        .entries
        .retain(|entry| entry.field_id != "world.account_aliases");
    assert!(
        consume_account(
            &incomplete,
            &block,
            &absent,
            block.dataspace_catalog(),
            &budget,
            |_, _, _, _| Ok(())
        )
        .is_err()
    );
    let alias =
        AccountAlias::from_literal(&name.canonical_text(), block.dataspace_catalog()).unwrap();
    block.account_rekey_records.remove(alias);
    let changed = snapshot(&block, &budget);
    assert!(
        consume_account(
            &changed.snapshot,
            &block,
            &name,
            block.dataspace_catalog(),
            &budget,
            |_, _, _, _| Ok(())
        )
        .is_err()
    );
    assert!(
        consume_account(
            &captured.snapshot,
            &block,
            &name,
            block.dataspace_catalog(),
            &budget,
            |_, _, _, _| Ok(())
        )
        .is_err()
    );
}
#[test]
fn fee_originals_return_present_zero_bucket_and_only_selected_program_values() {
    let (world, program, fee) = fee_world(true);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let block = world.block();
    let captured = snapshot(&block, &budget);
    consume_fee(
        &captured.snapshot,
        &block,
        &program,
        &fee,
        &budget,
        |_,
         assets,
         programs,
         revisions,
         enrollments,
         vaults,
         counters,
         _,
         definition,
         source,
         selected,
         rv,
         ev,
         vv| {
            assert_eq!(assets.len(), 1);
            assert_eq!(assets[0].scope(), &AssetBalanceScope::Global);
            assert_eq!(programs.len(), 1);
            assert!(selected.is_none());
            assert!(
                revisions.is_empty()
                    && enrollments.is_empty()
                    && vaults.is_empty()
                    && counters.is_empty()
            );
            assert!(rv.is_empty() && ev.is_empty() && vv.is_empty());
            assert_eq!(
                definition.balance_scope_policy(),
                AssetBalancePolicy::Global
            );
            assert_eq!(source.as_ref(), &0_u32.into());
            Ok(())
        },
    )
    .unwrap();
    let mut incomplete = captured.snapshot.clone();
    incomplete
        .entries
        .retain(|entry| entry.field_id != "world.fee_sponsor_programs");
    assert!(
        consume_fee(
            &incomplete,
            &block,
            &program,
            &fee,
            &budget,
            |_, _, _, _, _, _, _, _, _, _, _, _, _, _| Ok(())
        )
        .is_err()
    );
}
#[test]
fn missing_fee_bucket_is_refused_and_changed_funding_original_is_not_a_pre_tail_value() {
    let (world, program, fee) = fee_world(false);
    let budget = AllocationBudget::new(16 * 1024 * 1024);
    let block = world.block();
    let captured = snapshot(&block, &budget);
    let called = Cell::new(false);
    let error = consume_fee(
        &captured.snapshot,
        &block,
        &program,
        &fee,
        &budget,
        |_, _, _, _, _, _, _, _, _, _, _, _, _, _| {
            called.set(true);
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("absence is not zero"));
    assert!(!called.get());
    let (world, program, fee) = fee_world(true);
    let mut block = world.block();
    let captured = snapshot(&block, &budget);
    let (id, value) = Asset::new(
        AssetId::with_scope(
            fee.clone(),
            program.sponsor.clone(),
            AssetBalanceScope::Global,
        ),
        1_u32,
    )
    .into_key_value();
    block.assets.insert(id, value);
    assert!(
        consume_fee(
            &captured.snapshot,
            &block,
            &program,
            &fee,
            &budget,
            |_, _, _, _, _, _, _, _, _, _, _, _, _, _| Ok(())
        )
        .is_err()
    );
}
#[test]
fn certified_fee_cut_requires_existing_read_root_and_releases_budget_on_refusal() {
    let (mut world, program, fee) = fee_world(true);
    let reader = program.sponsor.clone();
    world.account_permissions.insert(
        reader.clone(),
        BTreeSet::from([iroha_executor_data_model::permission::query::CanReadAllLedgerData.into()]),
    );
    let domain = DomainId::try_new("funding", "universal").unwrap();
    let mut config = TestChainConfig::new(world, 1_000);
    config.genesis_instructions = vec![Register::domain(Domain::new(domain)).into()];
    let mut chain = CertifiedTestChain::start(config).unwrap();
    chain.commit_at(2_000, vec![]);
    let tip = chain.committed(2);
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    chain
        .state()
        .with_native_global_fee_originals_v1(
            &tip,
            &reader,
            &program,
            &fee,
            &budget,
            |snapshot, _, _, _, _, _, _, _, _, _, _, _, _, _| {
                assert_eq!(
                    snapshot.root().unwrap(),
                    tip.commitment().execution.world_state_root
                );
                assert_eq!(
                    snapshot.schema_hash,
                    State::native_world_schema_hash_v1().unwrap()
                );
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    let called = Cell::new(false);
    assert!(
        chain
            .state()
            .with_native_global_fee_originals_v1(
                &tip,
                chain.genesis_account(),
                &program,
                &fee,
                &budget,
                |_, _, _, _, _, _, _, _, _, _, _, _, _, _| {
                    called.set(true);
                    Ok(())
                }
            )
            .is_err()
    );
    assert!(!called.get());
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(
        chain
            .state()
            .with_native_global_fee_originals_v1(
                &tip,
                &reader,
                &program,
                &fee,
                &AllocationBudget::new(0),
                |_, _, _, _, _, _, _, _, _, _, _, _, _, _| Ok(())
            )
            .is_err()
    );
    chain.commit_at(3_000, vec![]);
    assert!(
        chain
            .state()
            .with_native_global_fee_originals_v1(
                &tip,
                &reader,
                &program,
                &fee,
                &budget,
                |_, _, _, _, _, _, _, _, _, _, _, _, _, _| Ok(())
            )
            .is_err()
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn certified_account_cut_resolves_nondefault_protected_catalog_and_never_infers_a_default() {
    use crate::state::{StateReadOnly as _, WorldReadOnly as _, derive_committee_key_id};
    use iroha_crypto::bls_normal_pop_prove;
    use iroha_data_model::{
        consensus::{ConsensusKeyRecord, ConsensusKeyStatus},
        isi::{InstructionBox, SetParameter, consensus_keys::RegisterConsensusKey},
        nexus::{
            LaneConfig, LaneLifecycleParameterV1, LaneVisibility, NexusCatalogTransitionV1,
            NexusRuntimeCatalogV1, RuntimeDataSpaceAdditionV1, RuntimeLaneManifestV1,
            dataspace_catalog_hash,
        },
        parameter::Parameter,
    };
    use iroha_executor_data_model::permission::{
        governance::CanManageConsensusKeys, parameter::CanSetParameters,
    };
    use iroha_model_base::{peer::PeerId, topology::LaneId};
    use iroha_primitives::{json::Json, numeric::Quantity};
    let mut world = World::new();
    let account = owner(49);
    let native_hash = Hash::new(b"explicit synthetic physical dataspace original");
    let dataspace = DataSpaceId::from_hash(native_hash.as_ref());
    let addition = RuntimeDataSpaceAdditionV1 {
        descriptor: DataSpaceMetadata {
            id: dataspace,
            alias: "is2".into(),
            description: None,
            fault_tolerance: 1,
        },
        manifest_hash: *native_hash.as_ref(),
    };
    let baseline = DataSpaceCatalog::default();
    let catalog = DataSpaceCatalog::new(vec![
        baseline.entries()[0].clone(),
        addition.descriptor.clone(),
    ])
    .unwrap();
    let name: AccountAliasName = "retail@leumi.is2".parse().unwrap();
    let alias = AccountAlias::from_literal(&name.canonical_text(), &catalog).unwrap();
    world.accounts.insert(
        account.clone(),
        Owned::new(AccountDetails::new(
            Metadata::default(),
            Some(alias.clone()),
            None,
            vec![],
        )),
    );
    world.account_permissions.insert(
        account.clone(),
        BTreeSet::from([
            iroha_executor_data_model::permission::query::CanReadAllLedgerData.into(),
            CanSetParameters.into(),
        ]),
    );
    world.account_aliases.insert(alias.clone(), account.clone());
    world.account_rekey_records.insert(
        alias.clone(),
        AccountRekeyRecord::new(alias.clone(), account.clone()),
    );
    let lease = NameRecordV1::new(
        crate::sns::selector_for_account_alias(&alias, &catalog).unwrap(),
        account.clone(),
        vec![],
        0,
        1,
        1_000_000,
        2_000_000,
        3_000_000,
        Metadata::default(),
    )
    .encode();
    world.smart_contract_state.insert(
        crate::sns::record_storage_key(
            &crate::sns::selector_for_account_alias(&alias, &catalog).unwrap(),
        ),
        lease.clone(),
    );
    // A World-only catalog is not an admitted runtime transition. Keep the
    // actual Universal baseline, register real synthetic lane Committee custody
    // in signed genesis, then execute the typed catalog transition at H2.
    let mut validators = (0xC1_u8..=0xC4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    validators.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    for key in &validators {
        let id = AccountId::new(key.public_key().clone());
        let (id, value) = Account::new(id.clone()).build(&id).into_key_value();
        world.accounts.insert(id, value);
    }
    let mut config = TestChainConfig::new(world, 1_000);
    // RegisterConsensusKey requires this exact permission even in signed genesis.
    // Keep the original genesis signer separate from the alias-bearing retail owner.
    config.world.account_permissions.insert(
        AccountId::new(config.genesis_key.public_key().clone()),
        BTreeSet::from([CanManageConsensusKeys.into()]),
    );
    config.validator_keys = Some(validators.clone());
    config.genesis_instructions = validators
        .iter()
        .map(|key| {
            let id = derive_committee_key_id(key.public_key());
            RegisterConsensusKey {
                id: id.clone(),
                record: ConsensusKeyRecord {
                    id,
                    public_key: key.public_key().clone(),
                    pop: Some(bls_normal_pop_prove(key.private_key()).unwrap()),
                    activation_height: 1,
                    expiry_height: None,
                    replaces: None,
                    status: ConsensusKeyStatus::Active,
                },
            }
            .into()
        })
        .collect();
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.staking.public_validator_mode =
        iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    config.nexus = Some(nexus);
    let mut chain = CertifiedTestChain::start(config).unwrap();
    let members = validators
        .iter()
        .map(|key| {
            norito::json!({
                "validator": (AccountId::new(key.public_key().clone()).to_string()),
                "peer_id": (PeerId::new(key.public_key().clone()).to_string())
            })
        })
        .collect::<Vec<_>>();
    let transition = {
        let view = chain.state().view();
        assert_eq!(view.nexus().configured_dataspace_catalog, baseline);
        assert!(view.runtime_catalog_hash().unwrap().is_none());
        NexusCatalogTransitionV1 {
            dataspace_retirements: Vec::new(),
            lane_retirements: Vec::new(),
            version: NexusCatalogTransitionV1::VERSION,
            expected_catalog_hash: LaneLifecycleParameterV1::catalog_hash(
                &view.nexus().lane_catalog,
            ),
            expected_incarnation_root: LaneLifecycleParameterV1::incarnation_root(
                &LaneLifecycleParameterV1::canonical_incarnations(
                    &view.nexus().lane_catalog,
                    &chain.state().lane_incarnations_snapshot(),
                )
                .unwrap(),
            ),
            expected_runtime_catalog_hash: view.runtime_catalog_hash().unwrap(),
            dataspace_additions: vec![addition.clone()],
            lane_additions: vec![LaneConfig {
                id: LaneId::new(1),
                alias: "is2".into(),
                dataspace_id: dataspace,
                visibility: LaneVisibility::Restricted,
                ..LaneConfig::default()
            }],
            manifest_additions: vec![RuntimeLaneManifestV1 {
                lane_id: LaneId::new(1),
                manifest: Json::new(norito::json!({
                    "lane": "is2", "version": 1,
                    "validators": members,
                    "quorum": 3
                })),
            }],
        }
    };
    let authority = KeyPair::from_seed(vec![49; 32], Algorithm::Ed25519);
    assert_eq!(AccountId::new(authority.public_key().clone()), account);
    let signed_transition = chain.sign(
        &authority,
        [InstructionBox::from(SetParameter::new(Parameter::Custom(
            transition.into_custom_parameter().unwrap(),
        )))],
        1_001,
    );
    assert_eq!(chain.commit_at(2_000, vec![signed_transition]), vec![true]);
    chain.commit_at(3_000, vec![]);
    let tip = chain.committed(3);
    {
        let view = chain.state().view();
        assert_eq!(view.nexus().configured_dataspace_catalog, baseline);
        assert_eq!(view.nexus().dataspace_catalog, catalog);
        let parameter = view
            .world()
            .parameters()
            .custom()
            .get(&NexusRuntimeCatalogV1::parameter_id())
            .unwrap();
        let runtime = NexusRuntimeCatalogV1::from_custom_parameter(parameter)
            .unwrap()
            .unwrap();
        assert_eq!(
            runtime.baseline_dataspaces_hash,
            dataspace_catalog_hash(&baseline)
        );
        assert_eq!(runtime.dataspaces, vec![addition]);
    }
    let budget = AllocationBudget::new(48 * 1024 * 1024);
    // A raw World overlay defaults to Universal, which must never drive this selection.
    assert!(
        chain
            .state()
            .world
            .block()
            .dataspace_catalog()
            .by_alias("is2")
            .is_none()
    );
    chain
        .state()
        .with_native_account_alias_originals_v1(
            &tip,
            &account,
            &name,
            &budget,
            |snapshot, selected, keys, original| {
                assert_eq!(selected.dataspace, dataspace);
                assert_eq!(keys, &[selected]);
                let (bound, rekey, value, actual_lease) = original.unwrap();
                assert_eq!(bound, &account);
                assert_eq!(&rekey.label, selected);
                assert_eq!(value.as_ref().label(), Some(selected));
                assert_eq!(actual_lease, lease);
                assert_eq!(
                    snapshot.root().unwrap(),
                    tip.commitment().execution.world_state_root
                );
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(
        chain
            .state()
            .with_native_account_alias_originals_v1(
                &tip,
                &account,
                &name,
                &AllocationBudget::new(1),
                |_, _, _, _| Ok(())
            )
            .is_err()
    );
}
