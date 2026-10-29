//! Governed runtime change sets through the public node API
//! (`specs/live_runtime_upgrades.md` §6): the baseline selection executes v1 programs, and a
//! change set reaches SORA Parliament only from an account holding `CanProposeRuntimeChangeSet`,
//! under a committed notice policy, with at least that notice.

use iroha_core::smartcontracts::Execute;
use iroha_core::{
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    prelude::World,
    query::store::LiveQueryStore,
    state::{State, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    NetworkId,
    isi::governance::ProposeRuntimeUpgradeProposal,
    parameter::{Parameter, RuntimeUpgradePolicyV1},
    prelude::*,
    runtime::{
        definition::RestrictionEntryV1,
        lifecycle::{
            ChangeSetV1, RestrictionKindV1, RootScopeV1, RuntimeNoticeV1, RuntimeUpgradeActionV1,
            SelectionTransitionV1,
        },
    },
};
use iroha_executor_data_model::permission::governance::CanProposeRuntimeChangeSet;
use iroha_model_base::{chain::ChainId, domain::DomainId};
use ivm::ProgramMetadata;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
use std::{borrow::Cow, num::NonZeroU64, sync::Arc};

const TEST_GAS_LIMIT: u64 = 1_000_000;

fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
    let mut program = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1,
        abi_version,
    }
    .encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    program
}

fn world_with_account() -> (World, AccountId, KeyPair) {
    let key_pair = KeyPair::random();
    let account_id = AccountId::of(key_pair.public_key().clone());
    let domain =
        Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
    (world, account_id, key_pair)
}

/// A change set that deprecates the baseline's native bundle from `effective_height`.
fn deprecate_bundle(network: NetworkId, effective_height: u64) -> ChangeSetV1 {
    let baseline = iroha_core::runtime::release_catalog()
        .expect("release catalog")
        .baseline
        .clone();
    let bundle = baseline.bindings[0].implementation;
    let mut successor = baseline.clone();
    successor.restrictions.push(RestrictionEntryV1 {
        definition: bundle,
        restriction: RestrictionKindV1::Deprecated.at(effective_height),
    });
    successor.restrictions.sort_by_key(|entry| entry.definition);
    ChangeSetV1 {
        network,
        root: RootScopeV1::Universal,
        predecessor: baseline.id().expect("baseline identity"),
        successor: successor.id().expect("successor identity"),
        effective_height,
        publish: Vec::new(),
        transitions: vec![SelectionTransitionV1::Restrict {
            definition: bundle,
            restriction: RestrictionKindV1::Deprecated,
        }],
        qualification: [0x5A; 32],
    }
}

fn policy(blocks: u64) -> RuntimeUpgradePolicyV1 {
    let notice = RuntimeNoticeV1 {
        blocks,
        ms: 0,
        approval_cushion_ms: 0,
    };
    RuntimeUpgradePolicyV1 {
        activation: notice,
        disable: notice,
    }
}

#[test]
fn baseline_selection_executes_v1_programs() {
    use iroha_core::{
        smartcontracts::ivm::cache::IvmCache,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    let (world, account_id, key_pair) = world_with_account();
    let mut config = TestChainConfig::new(world, 0);
    config.chain_id = ChainId::from("chain");
    let chain = CertifiedTestChain::start(config).expect("signed genesis");
    let state = chain.state();
    let network_id = *state.network_id_ref();
    let nexus = state.nexus_snapshot();
    state.install_lane_manifests_for_testing(&Arc::new(
        LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance),
    ));
    let genesis = chain.genesis().header();
    let header = iroha_data_model::block::BlockHeader::new(
        nonzero!(2_u64),
        Some(genesis.hash()),
        None,
        genesis
            .creation_time_ms
            .checked_add(1)
            .expect("fixture time"),
        0,
    );
    let mut block = state.block(header);
    let mut ivm_cache = IvmCache::new();
    let execute = |block: &mut _, abi_version, ivm_cache: &mut IvmCache| {
        let tx = iroha_data_model::transaction::TransactionBuilder::new(
            network_id,
            account_id.clone(),
            FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(TEST_GAS_LIMIT)),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(
            minimal_ivm_program(abi_version),
        )))
        .sign(key_pair.private_key());
        iroha_core::tx::execute_component_transaction_for_testing(
            block,
            AcceptedTransaction::new_unchecked(Cow::Owned(tx)),
            ivm_cache,
            None,
        )
    };
    let result = execute(&mut block, 1, &mut ivm_cache);
    assert!(
        result.is_ok(),
        "the baseline host profile runs v1: {result:?}"
    );
    let result = execute(&mut block, 2, &mut ivm_cache);
    assert!(
        result.is_err(),
        "no selected host profile runs ABI version 2: {result:?}"
    );
}

#[test]
fn change_set_proposals_need_the_permission_a_committed_policy_and_its_notice() {
    let (world, account_id, _) = world_with_account();
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let network_id = *state.network_id_ref();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let propose = |effective_height| ProposeRuntimeUpgradeProposal {
        action: RuntimeUpgradeActionV1::Schedule(deprecate_bundle(network_id, effective_height)),
    };
    let rejection = |result: Result<(), _>| format!("{:?}", result.expect_err("rejected"));

    let error = rejection(propose(100).execute(&account_id, &mut stx));
    assert!(
        error.contains("CanProposeRuntimeChangeSet required"),
        "{error}"
    );
    Grant::account_permission(
        Permission::from(CanProposeRuntimeChangeSet),
        account_id.clone(),
    )
    .execute(&account_id, &mut stx)
    .expect("grant the proposal permission");
    let error = rejection(propose(100).execute(&account_id, &mut stx));
    assert!(
        error.contains("runtime upgrade policy is not committed"),
        "{error}"
    );

    let mut shorter_disable = policy(10);
    shorter_disable.disable.blocks = 9;
    let malformed = iroha_data_model::parameter::CustomParameter::new(
        RuntimeUpgradePolicyV1::parameter_id().clone(),
        iroha_primitives::json::Json::new(shorter_disable),
    );
    let error =
        rejection(SetParameter::new(Parameter::Custom(malformed)).execute(&account_id, &mut stx));
    assert!(
        error.contains("disabling must give at least the notice of an activation"),
        "{error}"
    );
    SetParameter::new(Parameter::Custom(
        policy(10).to_custom_parameter().expect("valid policy"),
    ))
    .execute(&account_id, &mut stx)
    .expect("commit the notice policy");

    let error = rejection(propose(10).execute(&account_id, &mut stx));
    assert!(
        error.contains("takes effect before its notice of 10 blocks"),
        "{error}"
    );
    propose(11)
        .execute(&account_id, &mut stx)
        .expect("a change set with the committed notice reaches Parliament");
    let proposals: Vec<_> = stx
        .world
        .governance_proposals()
        .iter()
        .filter_map(|(_, record)| record.as_runtime_upgrade().cloned())
        .collect();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].proposal_operator, account_id);
    assert_eq!(
        proposals[0].notice,
        policy(10).disable,
        "deprecation takes the disable notice"
    );
    assert!(
        stx.world.runtime_change_sets().iter().next().is_none(),
        "nothing is scheduled before Parliament approves"
    );
    assert_eq!(
        *stx.world.runtime_selection(),
        iroha_core::runtime::release_catalog()
            .expect("release catalog")
            .baseline,
        "a proposal never changes the selection"
    );
}
