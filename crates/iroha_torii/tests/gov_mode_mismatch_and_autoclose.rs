#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![doc = "Governance mode-mismatch and auto-close tests hosted in Torii crate (app-level).\nSkipped by default; enable with `IROHA_RUN_IGNORED=1`."]
#![cfg(all(
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]

#[path = "../../iroha_core/tests/zk_testkit.rs"]
mod zk_testkit;
use std::env;

use iroha_config::parameters::actual::VerifyingKeyRef;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute as _,
    state::{State, StateBlock, StateTransaction, World},
};
use iroha_data_model::{
    block::BlockHeader,
    events::data::{
        DataEvent,
        governance::{
            GovernanceEvent, GovernanceProposalApproved, GovernanceProposalRejected,
            GovernanceReferendumClosed, GovernanceReferendumOpened,
        },
    },
    isi::{
        governance::{CastPlainBallot, CastZkBallot, ProposeDeployContract, VotingMode},
        verifying_keys::RegisterVerifyingKey,
    },
    permission::Permission,
    prelude::Grant,
};
use iroha_primitives::json::Json;
use nonzero_ext::nonzero;

const CONTRACT_NAMESPACE: &str = "apps";
const CONTRACT_ID: &str = "demo.contract";
const BALLOT_SCOPE_ANY: &str = "any";
const DEFAULT_ABI_VERSION: &str = "1";

fn run_or_skip(tag: &str) -> bool {
    if env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1") {
        true
    } else {
        eprintln!("Skipping: {tag}. Set IROHA_RUN_IGNORED=1 to run.");
        false
    }
}

fn new_state() -> State {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    State::new_for_testing(World::default(), kura, query_handle)
}

fn block_header(height: u64) -> BlockHeader {
    BlockHeader::new(nonzero!(height), None, None, None, 0, 0)
}

fn grant_permission(
    stx: &mut StateTransaction<'_, '_>,
    authority: &iroha_data_model::account::AccountId,
    target: &iroha_data_model::account::AccountId,
    permission: Permission,
    label: &str,
) {
    Grant::account_permission(permission, target.clone())
        .execute(authority, stx)
        .unwrap_or_else(|_| panic!("{label}"));
}

fn contract_proposal(mode: VotingMode) -> ProposeDeployContract {
    ProposeDeployContract {
        namespace: CONTRACT_NAMESPACE.into(),
        contract_id: CONTRACT_ID.into(),
        code_hash_hex: "aa".repeat(32),
        abi_hash_hex: "bb".repeat(32),
        abi_version: DEFAULT_ABI_VERSION.into(),
        window: None,
        mode: Some(mode),
        manifest_provenance: None,
    }
}

fn can_propose_contract_permission() -> Permission {
    Permission::new(
        "CanProposeContractDeployment"
            .parse()
            .expect("valid permission"),
        Json::from(CONTRACT_ID),
    )
}

fn can_submit_ballot_permission(scope: &str) -> Permission {
    Permission::new(
        "CanSubmitGovernanceBallot"
            .parse()
            .expect("valid permission"),
        Json::from(scope),
    )
}

fn can_manage_vk_permission() -> Permission {
    Permission::new(
        "CanManageVerifyingKeys".parse().expect("valid permission"),
        Json::new(()),
    )
}

fn referendum_id(state: &State) -> String {
    state
        .view()
        .world()
        .governance_referenda()
        .iter()
        .next()
        .map(|(id, _)| id.clone())
        .expect("referendum id")
}

fn proposal_id(state: &State) -> [u8; 32] {
    state
        .view()
        .world()
        .governance_proposals()
        .iter()
        .next()
        .map(|(id, _)| *id)
        .expect("proposal id")
}

fn governance_events(block: &mut StateBlock<'_>) -> Vec<GovernanceEvent> {
    block
        .world
        .take_external_events()
        .into_iter()
        .filter_map(|event| match event.as_data_event() {
            Some(DataEvent::Governance(ev)) => Some(ev.clone()),
            _ => None,
        })
        .collect()
}

fn plain_ballot(
    referendum_id: &str,
    owner: &iroha_data_model::account::AccountId,
    amount: u128,
    duration_blocks: u64,
    direction: u8,
) -> CastPlainBallot {
    CastPlainBallot {
        referendum_id: referendum_id.to_string(),
        owner: owner.clone(),
        amount,
        duration_blocks,
        direction,
    }
}

fn zk_ballot(referendum_id: &str, proof_b64: &str, public_inputs_json: &str) -> CastZkBallot {
    CastZkBallot {
        election_id: referendum_id.to_string(),
        proof_b64: proof_b64.to_string(),
        public_inputs_json: public_inputs_json.to_string(),
    }
}

fn zk_public_inputs(owner: &iroha_data_model::account::AccountId) -> String {
    norito::json::to_json(
        &norito::json::object([
            (
                "nullifier",
                norito::json::to_value(&"99".repeat(32)).expect("serialize nullifier"),
            ),
            (
                "owner",
                norito::json::to_value(&owner.to_string()).expect("serialize owner"),
            ),
        ])
        .expect("serialize public inputs"),
    )
    .expect("serialize public inputs")
}

fn random_authority() -> iroha_data_model::account::AccountId {
    let kp = iroha_crypto::KeyPair::random();
    iroha_data_model::account::AccountId::of(kp.public_key().clone())
}

#[test]
fn torii_plain_ballot_rejected_on_zk_referendum() {
    if !run_or_skip("torii mode mismatch (Plain on Zk) gated.") {
        return;
    }

    let mut state = new_state();
    let alice = random_authority();
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    state.set_gov(cfg);

    let mut block = state.block(block_header(1));
    {
        let mut tx = block.transaction();
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_propose_contract_permission(),
            "grant propose",
        );
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_submit_ballot_permission(BALLOT_SCOPE_ANY),
            "grant ballot",
        );
        contract_proposal(VotingMode::Zk)
            .execute(&alice, &mut tx)
            .expect("propose");
        tx.apply();
    }

    let rid = referendum_id(&state);
    {
        let mut tx = block.transaction();
        let ballot = plain_ballot(&rid, &alice, 1_000, 10, 0);
        let err = ballot.execute(&alice, &mut tx).unwrap_err();
        assert!(format!("{err}").contains("referendum mode mismatch"));
        tx.apply();
    }

    let events = governance_events(&mut block);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GovernanceEvent::BallotRejected(_)))
    );
}

#[test]
fn torii_zk_ballot_rejected_on_plain_referendum() {
    if !run_or_skip("torii mode mismatch (Zk on Plain) gated.") {
        return;
    }

    let mut state = new_state();
    let alice = random_authority();
    let bundle = zk_testkit::tiny_add_bundle();
    let mut cfg = state.gov.clone();
    cfg.plain_voting_enabled = true;
    let vk_name = bundle.vk_id.name.clone();
    let backend = bundle.backend.to_string();
    cfg.vk_ballot = Some(VerifyingKeyRef {
        backend: backend.clone(),
        name: vk_name.clone(),
    });
    cfg.vk_tally = Some(VerifyingKeyRef {
        backend,
        name: vk_name,
    });
    state.set_gov(cfg);

    let mut block = state.block(block_header(1));
    {
        let mut tx = block.transaction();
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_propose_contract_permission(),
            "grant propose",
        );
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_submit_ballot_permission(BALLOT_SCOPE_ANY),
            "grant ballot",
        );
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_manage_vk_permission(),
            "grant manage vk",
        );
        RegisterVerifyingKey {
            id: bundle.vk_id.clone(),
            record: bundle.vk_record.clone(),
        }
        .execute(&alice, &mut tx)
        .expect("register vk");
        contract_proposal(VotingMode::Plain)
            .execute(&alice, &mut tx)
            .expect("propose");
        tx.apply();
    }

    let rid = referendum_id(&state);
    let proof_b64 = bundle.proof_b64.clone();
    let public_inputs = zk_public_inputs(&alice);
    {
        let mut tx = block.transaction();
        let ballot = zk_ballot(&rid, &proof_b64, &public_inputs);
        let err = ballot.execute(&alice, &mut tx).unwrap_err();
        assert!(format!("{err}").contains("referendum mode mismatch"));
        tx.apply();
    }

    let events = governance_events(&mut block);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GovernanceEvent::BallotRejected(_)))
    );
}

#[test]
fn torii_referendum_auto_open_and_close_by_height() {
    if !run_or_skip("torii auto open/close test gated.") {
        return;
    }

    let mut state = new_state();
    let alice = random_authority();
    // Make windows short so we can tick quickly
    let mut cfg = state.gov.clone();
    cfg.min_enactment_delay = 1;
    cfg.window_span = 2; // open at H+1, close at H+2
    state.set_gov(cfg);

    {
        // H=1: propose a referendum (mode arbitrary)
        let mut block1 = state.block(block_header(1));
        let mut tx = block1.transaction();
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_propose_contract_permission(),
            "grant propose",
        );
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_submit_ballot_permission(BALLOT_SCOPE_ANY),
            "grant ballot",
        );
        contract_proposal(VotingMode::Plain)
            .execute(&alice, &mut tx)
            .expect("propose");
        tx.apply();
        // Drop block1 to release borrow before next block.
    }

    let pid = proposal_id(&state);
    let rid = referendum_id(&state);

    {
        // H=2: referendum auto-opens
        let mut block2 = state.block(block_header(2));
        let events = governance_events(&mut block2);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, GovernanceEvent::ReferendumOpened(_))),
            "expected ReferendumOpened at H=2"
        );

        // Cast an approving plain ballot at H=2 so decision at H=3 is Approved
        let mut tx = block2.transaction();
        let ballot = plain_ballot(&rid, &alice, 1, 100, 0);
        ballot.execute(&alice, &mut tx).expect("cast ballot");
        tx.apply();
    }

    {
        // H=3: referendum closes and proposal approved
        let mut block3 = state.block(block_header(3));
        let events = governance_events(&mut block3);
        let closed = events
            .iter()
            .any(|event| matches!(event, GovernanceEvent::ReferendumClosed(_)));
        let approved = events.iter().any(|event| {
            matches!(
                event,
                GovernanceEvent::ProposalApproved(GovernanceProposalApproved { id }) if id == &pid
            )
        });
        assert!(closed, "expected ReferendumClosed at H=3");
        assert!(
            approved,
            "expected ProposalApproved with matching id at H=3"
        );
    }
}

#[test]
fn torii_referendum_auto_close_reject_decision() {
    if !run_or_skip("torii auto-close reject test gated.") {
        return;
    }

    let mut state = new_state();
    let alice = random_authority();

    // Configure a short window
    let mut cfg = state.gov.clone();
    cfg.min_enactment_delay = 1;
    cfg.window_span = 2;
    state.set_gov(cfg);

    {
        // H=1: propose Plain referendum
        let mut block1 = state.block(block_header(1));
        let mut tx = block1.transaction();
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_propose_contract_permission(),
            "grant propose",
        );
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_submit_ballot_permission(BALLOT_SCOPE_ANY),
            "grant ballot",
        );
        contract_proposal(VotingMode::Plain)
            .execute(&alice, &mut tx)
            .expect("propose");
        tx.apply();
    }

    let pid = proposal_id(&state);
    let rid = referendum_id(&state);

    {
        // H=2: open and cast a rejecting ballot
        let mut block2 = state.block(block_header(2));
        let events = governance_events(&mut block2);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, GovernanceEvent::ReferendumOpened(_))),
            "expected ReferendumOpened at H=2"
        );

        let mut tx = block2.transaction();
        let ballot = plain_ballot(&rid, &alice, 1, 100, 1);
        ballot.execute(&alice, &mut tx).expect("cast reject");
        tx.apply();
    }

    {
        // H=3: close and expect ProposalRejected(pid)
        let mut block3 = state.block(block_header(3));
        let events = governance_events(&mut block3);
        let closed = events
            .iter()
            .any(|event| matches!(event, GovernanceEvent::ReferendumClosed(_)));
        let rejected = events.iter().any(|event| {
            matches!(
                event,
                GovernanceEvent::ProposalRejected(GovernanceProposalRejected { id }) if id == &pid
            )
        });
        assert!(closed, "expected ReferendumClosed at H=3");
        assert!(
            rejected,
            "expected ProposalRejected with matching id at H=3"
        );
    }
}

#[test]
fn torii_threshold_equal_approves_two_thirds() {
    if !run_or_skip("torii threshold equality test gated.") {
        return;
    }

    let mut state = new_state();
    let alice = random_authority();
    let bob = random_authority();

    // Configure 2/3 threshold and a short window
    let mut cfg = state.gov.clone();
    cfg.min_enactment_delay = 1;
    cfg.window_span = 2;
    cfg.approval_threshold_q_num = 2;
    cfg.approval_threshold_q_den = 3;
    state.set_gov(cfg);

    {
        // H=1: propose Plain referendum and grant permissions to both voters
        let mut block1 = state.block(block_header(1));
        let mut tx = block1.transaction();
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_propose_contract_permission(),
            "grant propose",
        );
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_submit_ballot_permission(BALLOT_SCOPE_ANY),
            "grant ballot alice",
        );
        grant_permission(
            &mut tx,
            &alice,
            &bob,
            can_submit_ballot_permission(BALLOT_SCOPE_ANY),
            "grant ballot bob",
        );
        contract_proposal(VotingMode::Plain)
            .execute(&alice, &mut tx)
            .expect("propose");
        tx.apply();
    }

    let pid = proposal_id(&state);
    let rid = referendum_id(&state);

    {
        // H=2: open and cast mixed ballots with approve:reject weights = 2:1 at conviction=2
        let mut block2 = state.block(block_header(2));
        // Drain open events to keep state clean
        let _ = governance_events(&mut block2);

        let mut tx = block2.transaction();
        let aye = plain_ballot(&rid, &alice, 4, 100, 0);
        aye.execute(&alice, &mut tx).expect("aye");
        let nay = plain_ballot(&rid, &bob, 1, 100, 1);
        nay.execute(&bob, &mut tx).expect("nay");
        tx.apply();
    }

    {
        // H=3: close; with 2/3 threshold and equality, expect approval
        let mut block3 = state.block(block_header(3));
        let events = governance_events(&mut block3);
        let closed = events
            .iter()
            .any(|event| matches!(event, GovernanceEvent::ReferendumClosed(_)));
        let approved = events.iter().any(|event| {
            matches!(
                event,
                GovernanceEvent::ProposalApproved(GovernanceProposalApproved { id }) if id == &pid
            )
        });
        assert!(closed, "expected ReferendumClosed at H=3");
        assert!(
            approved,
            "expected ProposalApproved with matching id at equality threshold 2/3"
        );
    }
}

#[test]
fn torii_threshold_below_rejects_two_thirds() {
    if !run_or_skip("torii threshold below test gated.") {
        return;
    }

    let mut state = new_state();
    let alice = random_authority();
    let bob = random_authority();

    // Configure 2/3 threshold and short window
    let mut cfg = state.gov.clone();
    cfg.min_enactment_delay = 1;
    cfg.window_span = 2;
    cfg.approval_threshold_q_num = 2;
    cfg.approval_threshold_q_den = 3;
    state.set_gov(cfg);

    {
        // H=1: propose referendum and grant ballot perms
        let mut block1 = state.block(block_header(1));
        let mut tx = block1.transaction();
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_propose_contract_permission(),
            "grant propose",
        );
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_submit_ballot_permission(BALLOT_SCOPE_ANY),
            "grant ballot alice",
        );
        grant_permission(
            &mut tx,
            &alice,
            &bob,
            can_submit_ballot_permission(BALLOT_SCOPE_ANY),
            "grant ballot bob",
        );
        contract_proposal(VotingMode::Plain)
            .execute(&alice, &mut tx)
            .expect("propose");
        tx.apply();
    }

    let pid = proposal_id(&state);
    let rid = referendum_id(&state);

    {
        // H=2: open and cast below-threshold ballots with approve:reject = 1:2 (weights)
        let mut block2 = state.block(block_header(2));
        let _ = governance_events(&mut block2);
        let mut tx = block2.transaction();
        let aye = plain_ballot(&rid, &alice, 1, 100, 0);
        aye.execute(&alice, &mut tx).expect("aye");
        let nay = plain_ballot(&rid, &bob, 4, 100, 1);
        nay.execute(&bob, &mut tx).expect("nay");
        tx.apply();
    }

    {
        // H=3: close; expect rejection at 2/3 threshold
        let mut block3 = state.block(block_header(3));
        let events = governance_events(&mut block3);
        let closed = events
            .iter()
            .any(|event| matches!(event, GovernanceEvent::ReferendumClosed(_)));
        let rejected = events.iter().any(|event| {
            matches!(
                event,
                GovernanceEvent::ProposalRejected(GovernanceProposalRejected { id }) if id == &pid
            )
        });
        assert!(closed, "expected ReferendumClosed at H=3");
        assert!(
            rejected,
            "expected ProposalRejected with matching id at 2/3 below-threshold"
        );
    }
}

#[test]
fn torii_min_turnout_rejects_on_auto_close() {
    if !run_or_skip("torii min_turnout test gated.") {
        return;
    }

    let mut state = new_state();
    let alice = random_authority();

    // Configure short window and min_turnout > 0 to force rejection when no ballots are cast
    let mut cfg = state.gov.clone();
    cfg.min_enactment_delay = 1;
    cfg.window_span = 2;
    cfg.min_turnout = 5; // require non-zero turnout
    state.set_gov(cfg);

    {
        // H=1: propose Plain referendum
        let mut block1 = state.block(block_header(1));
        let mut tx = block1.transaction();
        grant_permission(
            &mut tx,
            &alice,
            &alice,
            can_propose_contract_permission(),
            "grant propose",
        );
        contract_proposal(VotingMode::Plain)
            .execute(&alice, &mut tx)
            .expect("propose");
        tx.apply();
    }

    let pid = proposal_id(&state);

    {
        // H=2: observe open (no ballots cast)
        let mut block2 = state.block(block_header(2));
        let events = governance_events(&mut block2);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, GovernanceEvent::ReferendumOpened(_))),
            "expected ReferendumOpened at H=2"
        );
    }

    {
        // H=3: close; expect rejection due to turnout < min_turnout
        let mut block3 = state.block(block_header(3));
        let events = governance_events(&mut block3);
        let closed = events
            .iter()
            .any(|event| matches!(event, GovernanceEvent::ReferendumClosed(_)));
        let rejected = events.iter().any(|event| {
            matches!(
                event,
                GovernanceEvent::ProposalRejected(GovernanceProposalRejected { id }) if id == &pid
            )
        });
        assert!(closed, "expected ReferendumClosed at H=3");
        assert!(
            rejected,
            "expected ProposalRejected due to turnout < min_turnout"
        );
    }
}
