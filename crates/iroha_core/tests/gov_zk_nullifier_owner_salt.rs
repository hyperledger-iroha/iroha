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
use iroha_primitives::{json::Json, numeric::Numeric};
use mv::storage::StorageReadOnly;

fn canonical_abi_hex() -> String {
    hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))
}

#[test]
fn zk_ballot_nullifier_commit_duplicate_rejected() {
    use core::num::NonZeroU64;

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
    let domain: Domain = Domain::new("wonderland".parse().unwrap()).build(&alice_id);
    let acc = Account::new(alice_id.clone()).build(&alice_id);
    let escrow_acc = Account::new(escrow_id.clone()).build(&alice_id);
    let receiver_acc = Account::new(receiver_id.clone()).build(&alice_id);
    let def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
    let asset_def = AssetDefinition::numeric(def_id.clone()).build(&alice_id);
    let alice_asset = Asset::new(
        AssetId::new(def_id.clone(), alice_id.clone()),
        Numeric::new(1_000, 0),
    );
    let escrow_asset = Asset::new(
        AssetId::new(def_id.clone(), escrow_id.clone()),
        Numeric::new(0, 0),
    );
    let receiver_asset = Asset::new(
        AssetId::new(def_id.clone(), receiver_id.clone()),
        Numeric::new(0, 0),
    );
    let world = iroha_core::state::World::with_assets(
        [domain],
        [acc, escrow_acc, receiver_acc],
        [asset_def],
        [alice_asset, escrow_asset, receiver_asset],
        [],
    );
    let mut state = State::new_for_testing(world, kura, query_handle);
    // Install Halo2 verifying key defaults for governance
    let bundle = zk_testkit::add2inst_public_bundle(5, 8);
    let bundle_alt = zk_testkit::add2inst_public_bundle(6, 8);
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
    cfg.min_bond_amount = 1;
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
            contract_id: "demo.contract".to_string(),
        }
        .into();
        Grant::account_permission(p1, alice_id.clone())
            .execute(&alice_id, &mut stx)
            .expect("grant propose");
        let p2: Permission = CanSubmitGovernanceBallot {
            referendum_id: "any".to_string(),
        }
        .into();
        Grant::account_permission(p2, alice_id.clone())
            .execute(&alice_id, &mut stx)
            .expect("grant ballot");
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
            namespace: "apps".to_string(),
            contract_id: "demo.contract".to_string(),
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
        create
            .execute(&alice_id, &mut stx)
            .expect("create election");
        stx.apply();
    }

    let proof_b64 = bundle.proof_b64.clone();
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
        stx2.apply();
    }
    let locks_after = sblock
        .world
        .governance_locks()
        .get(&rid)
        .cloned()
        .expect("locks after slash");
    let rec = locks_after
        .locks
        .get(&alice_id)
        .expect("alice lock after slash");
    assert_eq!(rec.amount, 75);
    assert_eq!(rec.slashed, 25);
    let ledger = sblock
        .world
        .governance_slashes()
        .get(&rid)
        .cloned()
        .expect("slash ledger");
    let entry = ledger.slashes.get(&alice_id).expect("slash ledger entry");
    assert_eq!(entry.total_slashed, 25);
    assert_eq!(entry.total_restituted, 0);
    assert_eq!(
        entry.last_reason,
        iroha_data_model::events::data::governance::GovernanceSlashReason::DoubleVote
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
    assert_eq!(escrow_balance, Numeric::new(75, 0));
    assert_eq!(receiver_balance, Numeric::new(25, 0));

    // Changing the proof (commit) allows a second distinct ballot.
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
        election_id: rid.clone(),
        proof_b64: bundle_alt.proof_b64.clone(),
        public_inputs_json: norito::json::to_json(&public2).unwrap(),
    };
    {
        let mut stx3 = sblock.transaction();
        instr3
            .execute(&alice_id, &mut stx3)
            .expect("second ballot ok");
        stx3.apply();
    }

    // Check that we saw a BallotAccepted and a BallotRejected among events
    let events = sblock.world.take_external_events();
    let mut saw_accept = false;
    let mut saw_reject = false;
    for event in events {
        if let Some(DataEvent::Governance(ge)) = event.as_data_event() {
            match ge {
                GovernanceEvent::BallotAccepted(_) => saw_accept = true,
                GovernanceEvent::BallotRejected(_) => saw_reject = true,
                _ => {}
            }
        }
    }
    assert!(saw_accept, "expected at least one BallotAccepted event");
    assert!(saw_reject, "expected at least one BallotRejected event");
}
