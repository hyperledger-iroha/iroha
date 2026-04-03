#![doc = "Governance ZK ballot must fail when referendum is missing or outside window."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{
    Registrable,
    block::BlockHeader,
    domain::DomainId,
    isi::governance::CastZkBallot,
    permission::Permission,
    prelude::{Account, Domain, Grant},
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_test_samples::ALICE_ID;
use nonzero_ext::nonzero;

#[test]
fn zk_ballot_rejected_when_referendum_absent_or_out_of_window() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account: Account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let mut state =
        State::new_for_testing(World::with([domain], [account], []), kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.min_bond_amount = 0;
    gov_cfg.min_enactment_delay = 0;
    gov_cfg.window_span = 10;
    state.set_gov(gov_cfg);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    {
        let mut sblock = state.block(header);
        let mut stx = sblock.transaction();
        let ballot_perm: Permission = CanSubmitGovernanceBallot {
            referendum_id: "any".to_string(),
        }
        .into();
        Grant::account_permission(ballot_perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("grant ballot permission");
        stx.world.elections_mut().insert(
            "missing".to_string(),
            iroha_core::state::ElectionState {
                options: 1,
                eligible_root: [0u8; 32],
                start_ts: 0,
                end_ts: 0,
                finalized: false,
                tally: vec![0],
                ballot_nullifiers: Default::default(),
                ciphertexts: Vec::new(),
                vk_ballot: None,
                vk_ballot_commitment: None,
                vk_tally: None,
                vk_tally_commitment: None,
                domain_tag: "gov:ballot:v1".to_string(),
            },
        );

        // Missing referendum
        let ballot = CastZkBallot {
            election_id: "missing".to_string(),
            proof_b64: "AA==".to_string(),
            public_inputs_json: "{}".to_string(),
        };
        let err = ballot
            .clone()
            .execute(&ALICE_ID, &mut stx)
            .expect_err("ballot should fail when referendum missing");
        assert!(
            err.to_string().contains("referendum"),
            "unexpected error: {err}"
        );

        // Closed referendum
        stx.world.governance_referenda_mut().insert(
            "closed".to_string(),
            iroha_core::state::GovernanceReferendumRecord {
                h_start: 0,
                h_end: 10,
                status: iroha_core::state::GovernanceReferendumStatus::Closed,
                mode: iroha_core::state::GovernanceReferendumMode::Zk,
            },
        );
        stx.world.elections_mut().insert(
            "closed".to_string(),
            iroha_core::state::ElectionState {
                options: 1,
                eligible_root: [0u8; 32],
                start_ts: 0,
                end_ts: 0,
                finalized: false,
                tally: vec![0],
                ballot_nullifiers: Default::default(),
                ciphertexts: Vec::new(),
                vk_ballot: None,
                vk_ballot_commitment: None,
                vk_tally: None,
                vk_tally_commitment: None,
                domain_tag: "gov:ballot:v1".to_string(),
            },
        );
        let ballot_closed = CastZkBallot {
            election_id: "closed".to_string(),
            proof_b64: "AA==".to_string(),
            public_inputs_json: "{}".to_string(),
        };
        let err_closed = ballot_closed
            .execute(&ALICE_ID, &mut stx)
            .expect_err("ballot should fail when referendum closed");
        assert!(
            err_closed.to_string().contains("closed"),
            "unexpected error: {err_closed}"
        );
        stx.apply();
        sblock.commit().expect("commit block at H=1");
    }

    // Outside window
    let header_late = BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
    let mut sblock_late = state.block(header_late);
    let mut stx_late = sblock_late.transaction();
    stx_late.world.governance_referenda_mut().insert(
        "late".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 5,
            h_end: 6,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );
    stx_late.world.elections_mut().insert(
        "late".to_string(),
        iroha_core::state::ElectionState {
            options: 1,
            eligible_root: [0u8; 32],
            start_ts: 0,
            end_ts: 0,
            finalized: false,
            tally: vec![0],
            ballot_nullifiers: Default::default(),
            ciphertexts: Vec::new(),
            vk_ballot: None,
            vk_ballot_commitment: None,
            vk_tally: None,
            vk_tally_commitment: None,
            domain_tag: "gov:ballot:v1".to_string(),
        },
    );
    let ballot_late = CastZkBallot {
        election_id: "late".to_string(),
        proof_b64: "AA==".to_string(),
        public_inputs_json: "{}".to_string(),
    };
    let err_late = ballot_late
        .execute(&ALICE_ID, &mut stx_late)
        .expect_err("ballot should fail when outside window");
    assert!(
        err_late.to_string().contains("not active") || err_late.to_string().contains("referendum"),
        "unexpected error: {err_late}"
    );
    stx_late.apply();
    sblock_late.commit().expect("commit block at H=10");
}
