//! Citizenship bonding and gating tests for governance flows.

use crate::native_genesis::{certified_state, certified_state_with};
use iroha_config::parameters::actual::Governance;
use iroha_core::{
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
    sumeragi::test_chain::TestChainConfig,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    Registrable,
    asset::{Asset, AssetDefinition},
    block::BlockHeader,
    domain::Domain,
    isi::governance::{RegisterCitizen, UnregisterCitizen},
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;
/// Apply the authenticated genesis over `world` with a governance policy fixed before signing.
fn governed_state(world: World, configure: impl FnOnce(&mut Governance)) -> State {
    let mut governance = certified_state(World::default()).gov.clone();
    configure(&mut governance);
    let mut config = TestChainConfig::new(world, 0);
    config.governance = Some(governance);
    certified_state_with(config)
}
/// An ordinary fixture block after the authenticated genesis at height one.
fn ordinary_header(state: &State, height: u64) -> BlockHeader {
    BlockHeader::new(
        std::num::NonZeroU64::new(height).expect("non-zero height"),
        state.view().latest_block_hash(),
        None,
        height,
        0,
    )
}
/// Direct citizenship components execute under one explicit retained invocation, as a
/// signed call does.
fn citizenship_call() -> Hash {
    Hash::new(b"gov-citizenship-component")
}
fn build_world(def_id: &AssetDefinitionId) -> World {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account = iroha_data_model::account::Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let escrow_account = iroha_data_model::account::Account::new(BOB_ID.clone()).build(&BOB_ID);
    let asset_def = AssetDefinition::numeric(
        def_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&ALICE_ID);
    let alice_asset = Asset::new(
        AssetId::new(def_id.clone(), ALICE_ID.clone()),
        Quantity::from(1_000_u64),
    );
    let escrow_asset = Asset::new(
        AssetId::new(def_id.clone(), BOB_ID.clone()),
        Quantity::from(0_u64),
    );
    World::with_assets(
        [domain],
        [alice_account, escrow_account],
        [asset_def],
        [alice_asset, escrow_asset],
        [],
    )
}
#[test]
fn register_and_revoke_citizenship_moves_bond() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let state = governed_state(build_world(&def_id), |gov_cfg| {
        gov_cfg.citizenship_asset_id = def_id.clone();
        gov_cfg.citizenship_bond_amount = 50_u64.into();
        gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    });
    // Direct execution and transcript recording share the block witness recorder;
    // the State, whose signed genesis records its own witness, is built first.
    let _witness_guard = iroha_core::exec_witness::exec_witness_guard();
    // The first ordinary block after the authenticated genesis bonds the citizen.
    let bond_height = 2;
    let mut sblock = state.block(ordinary_header(&state, bond_height));
    let mut stx = sblock.transaction_for_fastpq_testing(citizenship_call());
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("citizen bond succeeds");
    let record = stx
        .world
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record stored");
    assert_eq!(record.owner, *ALICE_ID);
    assert_eq!(record.amount, Quantity::from(50_u64));
    assert_eq!(record.bonded_height, bond_height);
    let alice_asset_id = AssetId::new(def_id.clone(), ALICE_ID.clone());
    let escrow_asset_id = AssetId::new(def_id.clone(), BOB_ID.clone());
    assert_eq!(
        **stx.world.asset_mut(&alice_asset_id).expect("alice asset"),
        Quantity::from(950_u64)
    );
    assert_eq!(
        **stx.world.asset_mut(&escrow_asset_id).expect("escrow asset"),
        Quantity::from(50_u64)
    );
    UnregisterCitizen {
        owner: ALICE_ID.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("citizen unbond succeeds");
    assert!(stx.world.citizens().get(&*ALICE_ID).is_none());
    assert_eq!(
        **stx.world.asset_mut(&alice_asset_id).expect("alice asset"),
        Quantity::from(1_000_u64)
    );
    assert!(stx.world.assets().get(&escrow_asset_id).is_none());
}

#[test]
fn citizenship_gate_blocks_and_allows_governance() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    // The authenticated genesis authors the ordinary signing policy used by the
    // funding helper's account admission.
    let state = governed_state(build_world(&def_id), |gov_cfg| {
        gov_cfg.voting_asset_id = def_id.clone();
        gov_cfg.citizenship_asset_id = def_id.clone();
        gov_cfg.citizenship_bond_amount = 10_u64.into();
        gov_cfg.citizenship_escrow_account = BOB_ID.clone();
        gov_cfg.plain_voting_enabled = true;
        gov_cfg.min_bond_amount = 0_u64.into();
        gov_cfg.conviction_step_blocks = 10;
        gov_cfg.bond_escrow_account = iroha_test_samples::CARPENTER_ID.clone();
        gov_cfg.slash_receiver_account = iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone();
    });
    // Direct execution and transcript recording share the block witness recorder;
    // the State, whose signed genesis records its own witness, is built first.
    let _witness_guard = iroha_core::exec_witness::exec_witness_guard();
    let mut sblock = state.block(ordinary_header(&state, 2));
    let mut stx = sblock.transaction_for_fastpq_testing(citizenship_call());
    // Seed referendum and permissions.
    iroha_core::query::standalone_plain_test_fixture::fund_voter(
        &mut stx,
        &iroha_test_samples::ALICE_ID,
        1_000_000_u64.into(),
        0,
    );
    let plain_context = iroha_core::query::standalone_plain_test_fixture::context(&stx.gov, 0);
    stx.world.governance_referenda_mut().insert(
        "citizen-ref".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 1,
            // The H=2 ballot below locks through H=22; keep the referendum's
            // inclusive end within that exact lock lifetime.
            h_end: 22,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
            plain_context,
            plain_result: iroha_data_model::governance::conviction::PlainVotingResultV1::Pending,
        },
    );
    let ballot_perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: "citizen-ref".to_string(),
    }
    .into();
    Grant::account_permission(ballot_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant ballot permission");
    // Ballot should be rejected until citizenship is bonded.
    let ballot = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: "citizen-ref".to_string(),
        owner: ALICE_ID.clone(),
        amount: 10_u64.into(),
        duration_blocks: 20,
        direction: 0,
    };
    let ballot_err = ballot.clone().execute(&ALICE_ID, &mut stx).unwrap_err();
    assert!(format!("{ballot_err:?}").contains("citizenship bond required"));
    let rejected = stx.world.take_external_events();
    assert!(rejected.iter().any(|event| matches!(
        event.as_data_event(),
        Some(iroha_data_model::events::data::DataEvent::Governance(
            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(rej)
        )) if rej.referendum_id == "citizen-ref" && rej.reason.contains("citizenship")
    )));
    // Bond citizenship and retry.
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 10_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("citizen bond succeeds");
    ballot
        .execute(&ALICE_ID, &mut stx)
        .expect("ballot allowed once citizen bonded");
    assert!(
        stx.world
            .citizens()
            .get(&*ALICE_ID)
            .is_some_and(|rec| rec.amount >= Quantity::from(10_u64))
    );
    assert!(stx.world.governance_locks().get("citizen-ref").is_some());
    // Citizenship and voting hold separate real bonds; no extra quantity is minted on admission.
    let balance = |account: &iroha_data_model::account::AccountId| {
        stx.world
            .assets()
            .get(&AssetId::new(def_id.clone(), account.clone()))
            .map_or_else(Quantity::zero, |asset| asset.clone().into_inner())
    };
    assert_eq!(balance(&ALICE_ID), Quantity::from(980_u64));
    assert_eq!(balance(&BOB_ID), Quantity::from(10_u64));
    assert_eq!(
        balance(&iroha_test_samples::CARPENTER_ID),
        Quantity::from(10_u64)
    );
    assert_eq!(
        balance(&iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID),
        Quantity::zero()
    );
}
#[test]
fn citizenship_records_persist_across_transactions() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let state = governed_state(build_world(&def_id), |gov_cfg| {
        gov_cfg.citizenship_asset_id = def_id.clone();
        gov_cfg.citizenship_bond_amount = 50_u64.into();
        gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    });
    // Direct execution and transcript recording share the block witness recorder;
    // the State, whose signed genesis records its own witness, is built first.
    let _witness_guard = iroha_core::exec_witness::exec_witness_guard();
    let mut block_1 = state.block(ordinary_header(&state, 2));
    let mut stx_1 = block_1.transaction_for_fastpq_testing(citizenship_call());
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx_1)
    .expect("citizen bond succeeds");
    stx_1.apply();
    block_1
        .commit_world_overlay_for_testing()
        .expect("first block with citizen registration commits");
    let citizen_record = state
        .view()
        .world()
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record should persist after tx apply");
    assert_eq!(citizen_record.amount, Quantity::from(50_u64));
    let mut block_2 = state.block(ordinary_header(&state, 3));
    let stx_2 = block_2.transaction();
    assert_eq!(
        stx_2
            .world
            .citizens()
            .get(&*ALICE_ID)
            .expect("citizen record remains visible in the next transaction")
            .amount,
        Quantity::from(50_u64)
    );
}
#[test]
fn citizenship_top_up_preserves_the_original_bond_interval() {
    let def_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    );
    let state = governed_state(build_world(&def_id), |gov_cfg| {
        gov_cfg.citizenship_asset_id = def_id.clone();
        gov_cfg.citizenship_bond_amount = 50_u64.into();
        gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    });
    // Direct execution and transcript recording share the block witness recorder;
    // the State, whose signed genesis records its own witness, is built first.
    let _witness_guard = iroha_core::exec_witness::exec_witness_guard();
    // The first ordinary block after the authenticated genesis bonds the citizen.
    let bond_height = 2;
    let mut block_1 = state.block(ordinary_header(&state, bond_height));
    let mut stx_1 = block_1.transaction_for_fastpq_testing(citizenship_call());
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx_1)
    .expect("initial citizen bond succeeds");
    stx_1.apply();
    block_1
        .commit_world_overlay_for_testing()
        .expect("initial citizen bond block commits");
    let mut block_2 = state.block(ordinary_header(&state, bond_height + 1));
    let mut stx_2 = block_2.transaction_for_fastpq_testing(citizenship_call());
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 75_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx_2)
    .expect("citizenship top-up succeeds");
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 75_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx_2)
    .expect("same-amount citizenship registration is an idempotent no-op");
    let retained = stx_2
        .world
        .citizens()
        .get(&*ALICE_ID)
        .expect("topped-up citizen record");
    assert_eq!(retained.amount, Quantity::from(75_u64));
    assert_eq!(retained.bonded_height, bond_height);
    assert_eq!(
        **stx_2
            .world
            .asset_mut(&AssetId::new(def_id.clone(), ALICE_ID.clone()))
            .expect("alice citizenship asset"),
        Quantity::from(925_u64)
    );
    assert_eq!(
        **stx_2
            .world
            .asset_mut(&AssetId::new(def_id, BOB_ID.clone()))
            .expect("citizenship escrow asset"),
        Quantity::from(75_u64)
    );
}
