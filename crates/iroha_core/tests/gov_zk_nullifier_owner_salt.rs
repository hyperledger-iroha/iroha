#![doc = "ZK ballot nullifier derivation from (`chain_id`, `election_id`, commit).\nVerifies duplicate detection when the same proof is reused."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]
#![cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#![allow(clippy::too_many_lines, clippy::collapsible_match)]
mod zk_testkit;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, WorldReadOnly},
};
use iroha_data_model::{
    asset::{Asset, AssetDefinition},
    events::data::DataEvent,
    prelude::*,
};
use iroha_primitives::{json::Json, numeric::Quantity};
use mv::storage::StorageReadOnly;
fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}
fn proposal_contract_address(
    owner: &iroha_data_model::account::AccountId,
) -> iroha_data_model::smart_contract::ContractAddress {
    iroha_data_model::smart_contract::ContractAddress::derive(
        &"0000000000000000000000000000000000000000000000000000000000000001"
            .parse()
            .expect("canonical test network id"),
        owner,
        0,
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    )
    .expect("proposal contract address")
}
#[test]
fn zk_ballot_nullifier_commit_duplicate_rejected() {
    use core::{num::NonZeroU64, time::Duration};
    use iroha_data_model::{
        events::data::governance::GovernanceEvent,
        isi::governance::{CastZkBallot, ProposeDeployContract, VotingMode},
        permission::Permission,
        prelude::Grant,
    };
    use iroha_executor_data_model::permission::governance::{
        CanManageParliament, CanProposeContractDeployment, CanSubmitGovernanceBallot,
    };
    // Generate accounts and build a minimal world with governance assets
    let (alice_id, _alice_kp) = iroha_test_samples::gen_account_in("wonderland");
    let (escrow_id, _escrow_kp) = iroha_test_samples::gen_account_in("wonderland");
    let (receiver_id, _receiver_kp) = iroha_test_samples::gen_account_in("wonderland");
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: iroha_data_model::domain::DomainId =
        iroha_data_model::domain::DomainId::try_new("wonderland", "universal").unwrap();
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let acc = Account::new(alice_id.clone()).build(&alice_id);
    let escrow_acc = Account::new(escrow_id.clone()).build(&alice_id);
    let receiver_acc = Account::new(receiver_id.clone()).build(&alice_id);
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let asset_def = AssetDefinition::numeric(
        def_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&alice_id);
    let alice_asset = Asset::new(
        AssetId::new(def_id.clone(), alice_id.clone()),
        Quantity::from(1_000_u64),
    );
    let escrow_asset = Asset::new(
        AssetId::new(def_id.clone(), escrow_id.clone()),
        Quantity::from(0_u64),
    );
    let receiver_asset = Asset::new(
        AssetId::new(def_id.clone(), receiver_id.clone()),
        Quantity::from(0_u64),
    );
    let world = iroha_core::state::World::with_assets(
        [domain],
        [acc, escrow_acc, receiver_acc],
        [asset_def],
        [alice_asset, escrow_asset, receiver_asset],
        [],
    );
    let mut state = State::new_for_testing(world, kura, query_handle);
    state.zk.halo2.enabled = true;
    state.zk.verify_timeout = Duration::ZERO;
    // Install Halo2 verifying key defaults for governance
    let bundle = zk_testkit::vote_merkle8_bundle();
    let mut cfg = state.gov.clone();
    let vk_name = bundle.vk_id.name.clone();
    cfg.vk_ballot = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: bundle.backend.to_string(),
        name: vk_name.clone(),
    });
    cfg.vk_tally = Some(iroha_config::parameters::actual::VerifyingKeyRef {
        backend: bundle.backend.to_string(),
        name: vk_name,
    });
    cfg.min_enactment_delay = 0;
    cfg.window_span = 100;
    cfg.voting_asset_id = def_id.clone();
    cfg.bond_escrow_account = escrow_id.clone();
    cfg.slash_receiver_account = receiver_id.clone();
    cfg.min_bond_amount = 1_u64.into();
    cfg.conviction_step_blocks = 1;
    cfg.slash_double_vote_bps = 2_500;
    state.set_gov(cfg);
    // Create a block at H=1 and open a Zk referendum via ProposeDeployContract
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock = state.block(header);
    {
        let mut stx = sblock.transaction();
        // Grant permissions to ALICE to propose and submit ballots
        let p1: Permission = CanProposeContractDeployment {
            contract_address: proposal_contract_address(&alice_id),
        }
        .into();
        Grant::account_permission(p1, alice_id.clone())
            .execute(&alice_id, &mut stx)
            .expect("grant propose");
        let manage_vk = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
        Grant::account_permission(manage_vk, alice_id.clone())
            .execute(&alice_id, &mut stx)
            .expect("grant manage vk");
        let manage_parliament: Permission = CanManageParliament.into();
        Grant::account_permission(manage_parliament, alice_id.clone())
            .execute(&alice_id, &mut stx)
            .expect("grant manage parliament");
        iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
            id: bundle.vk_id.clone(),
            record: bundle.vk_record.clone(),
        }
        .execute(&alice_id, &mut stx)
        .expect("register vk");
        // Propose a Zk-mode referendum (explicit or default)
        let prop = ProposeDeployContract {
            contract_address: proposal_contract_address(&alice_id),
            code_hash_hex: "aa".repeat(32),
            abi_hash_hex: canonical_abi_hex(),
            abi_version: "1".to_string(),
            window: None,
            mode: Some(VotingMode::Zk),
            manifest_provenance: None,
        };
        prop.execute(&alice_id, &mut stx).expect("propose");
        stx.apply();
    }
    // Discover the referendum id (rid) created by proposal
    let rid = sblock
        .world
        .governance_referenda()
        .iter()
        .next()
        .map_or_else(|| "rid-zk".to_string(), |(k, _)| k.clone());
    let create = iroha_data_model::isi::zk::CreateElection {
        election_id: rid.clone(),
        options: 1,
        eligible_root: bundle.root_bytes(),
        start_ts: 0,
        end_ts: 0,
        vk_ballot: bundle.vk_id.clone(),
        vk_tally: bundle.vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    };
    {
        let mut stx = sblock.transaction();
        let submit_ballot: Permission = CanSubmitGovernanceBallot {
            referendum_id: rid.clone(),
        }
        .into();
        Grant::account_permission(submit_ballot, alice_id.clone())
            .execute(&alice_id, &mut stx)
            .expect("grant ballot for election");
        create
            .execute(&alice_id, &mut stx)
            .expect("create election");
        stx.apply();
    }
    let proof_b64 = bundle.proof_b64();
    let root_hint = hex::encode(bundle.root_bytes());
    let public = norito::json::object([
        (
            "owner",
            norito::json::to_value(&alice_id.to_string()).expect("serialize owner"),
        ),
        (
            "amount",
            norito::json::to_value(&100u64).expect("serialize amount"),
        ),
        (
            "duration_blocks",
            norito::json::to_value(&50u64).expect("serialize duration"),
        ),
        (
            "root_hint",
            norito::json::to_value(&root_hint).expect("serialize root_hint"),
        ),
    ])
    .expect("serialize public inputs");
    let instr1 = CastZkBallot {
        election_id: rid.clone(),
        proof_b64: proof_b64.clone(),
        public_inputs_json: norito::json::to_json(&public).unwrap(),
    };
    {
        let mut stx1 = sblock.transaction();
        instr1
            .execute(&alice_id, &mut stx1)
            .expect("first ballot ok");
        stx1.apply();
    }
    // Re-submit with the same commit → duplicate nullifier rejection
    let instr2 = CastZkBallot {
        election_id: rid.clone(),
        proof_b64: proof_b64.clone(),
        public_inputs_json: norito::json::to_json(&public).unwrap(),
    };
    {
        let mut stx2 = sblock.transaction();
        let e = instr2.execute(&alice_id, &mut stx2).unwrap_err();
        let s = format!("{e}");
        assert!(s.contains("duplicate ballot nullifier"));
        let rejected_events = stx2.world.take_external_events();
        assert!(rejected_events.iter().any(|event| matches!(
            event.as_data_event(),
            Some(DataEvent::Governance(GovernanceEvent::BallotRejected(rejected)))
                if rejected.reason.contains("duplicate ballot nullifier")
        )));
        // A rejected instruction rolls back its tentative slash and asset mutations.
    }
    let locks_after_rejection = sblock
        .world
        .governance_locks()
        .get(&rid)
        .cloned()
        .expect("locks after rejected duplicate");
    let rec = locks_after_rejection
        .locks
        .get(&alice_id)
        .expect("alice lock after rejected duplicate");
    assert_eq!(rec.amount, Quantity::from(100_u64));
    assert_eq!(rec.slashed, Quantity::zero());
    assert!(
        sblock.world.governance_slashes().get(&rid).is_none(),
        "rejected duplicate must not persist a slash ledger"
    );
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id.clone());
    let receiver_asset_id = AssetId::new(def_id.clone(), receiver_id.clone());
    let escrow_balance = sblock
        .world
        .assets()
        .get(&escrow_asset_id)
        .expect("escrow asset after slash")
        .clone()
        .0;
    let receiver_balance = sblock
        .world
        .assets()
        .get(&receiver_asset_id)
        .expect("receiver asset after slash")
        .clone()
        .0;
    assert_eq!(escrow_balance, Quantity::from(100_u64));
    assert_eq!(receiver_balance, Quantity::zero());
    // The same commitment is distinct in another election because the election id is part of the
    // nullifier domain separation.
    let rid_alt = format!("{rid}-alt");
    let create_alt = iroha_data_model::isi::zk::CreateElection {
        election_id: rid_alt.clone(),
        options: 1,
        eligible_root: bundle.root_bytes(),
        start_ts: 0,
        end_ts: 0,
        vk_ballot: bundle.vk_id.clone(),
        vk_tally: bundle.vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    };
    {
        let mut stx_alt = sblock.transaction();
        let submit_ballot: Permission = CanSubmitGovernanceBallot {
            referendum_id: rid_alt.clone(),
        }
        .into();
        Grant::account_permission(submit_ballot, alice_id.clone())
            .execute(&alice_id, &mut stx_alt)
            .expect("grant ballot for second election");
        create_alt
            .execute(&alice_id, &mut stx_alt)
            .expect("create second election");
        stx_alt.world.governance_referenda_mut().insert(
            rid_alt.clone(),
            iroha_core::state::GovernanceReferendumRecord {
                h_start: 0,
                h_end: 100,
                status: iroha_core::state::GovernanceReferendumStatus::Proposed,
                mode: iroha_core::state::GovernanceReferendumMode::Zk,
            },
        );
        stx_alt.apply();
    }
    let public2 = norito::json::object([
        (
            "owner",
            norito::json::to_value(&alice_id.to_string()).expect("serialize owner"),
        ),
        (
            "amount",
            norito::json::to_value(&100u64).expect("serialize amount"),
        ),
        (
            "duration_blocks",
            norito::json::to_value(&50u64).expect("serialize duration"),
        ),
        (
            "root_hint",
            norito::json::to_value(&root_hint).expect("serialize root_hint"),
        ),
    ])
    .expect("serialize public inputs");
    let instr3 = CastZkBallot {
        election_id: rid_alt,
        proof_b64: bundle.proof_b64(),
        public_inputs_json: norito::json::to_json(&public2).unwrap(),
    };
    {
        let mut stx3 = sblock.transaction();
        instr3
            .execute(&alice_id, &mut stx3)
            .expect("same commitment in second election must be accepted");
        stx3.apply();
    }
    // Both successful elections emit acceptance. The rejected duplicate event was observed only
    // inside its rolled-back transaction above.
    let events = sblock.world.take_external_events();
    let mut saw_accept = false;
    for event in events {
        if let Some(DataEvent::Governance(ge)) = event.as_data_event() {
            match ge {
                GovernanceEvent::BallotAccepted(_) => saw_accept = true,
                _ => {}
            }
        }
    }
    assert!(saw_accept, "expected at least one BallotAccepted event");
}
