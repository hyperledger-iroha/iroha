#![doc = "FinalizeReferendum rejects when a ZK election exists but the tally is not finalized."]
//! FinalizeReferendum rejects when a ZK election exists but the tally is not finalized.

use core::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{ElectionState, State, World},
};
use iroha_data_model::{block::BlockHeader, isi::governance::FinalizeReferendum};
use iroha_test_samples::ALICE_ID;

#[test]
fn finalize_referendum_rejects_unfinalized_zk_election() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let rid = "ref-finalize-zk".to_string();
    stx.world.elections_mut().insert(
        rid.clone(),
        ElectionState {
            options: 2,
            eligible_root: [0u8; 32],
            start_ts: 0,
            end_ts: 0,
            finalized: false,
            tally: vec![0, 0],
            ballot_nullifiers: std::collections::BTreeSet::default(),
            ciphertexts: Vec::new(),
            vk_ballot: None,
            vk_ballot_commitment: None,
            vk_tally: None,
            vk_tally_commitment: None,
            domain_tag: String::new(),
        },
    );
    stx.world.governance_referenda_mut().insert(
        rid.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 10,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );

    let instr = FinalizeReferendum {
        referendum_id: rid,
        proposal_id: [0u8; 32],
    };
    let err = instr.execute(&ALICE_ID, &mut stx).unwrap_err();
    assert!(err.to_string().contains("election tally not finalized"));
}
