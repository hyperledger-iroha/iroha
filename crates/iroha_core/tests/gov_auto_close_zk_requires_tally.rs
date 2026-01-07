//! ZK referenda auto-close should not emit decisions without a finalized tally.

use core::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        ElectionState, GovernanceReferendumMode, GovernanceReferendumRecord,
        GovernanceReferendumStatus, State, World,
    },
};
use iroha_data_model::{
    block::BlockHeader,
    events::data::{DataEvent, governance::GovernanceEvent},
};

#[test]
fn zk_referendum_auto_close_defers_decision_without_tally() {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);

    let rid = hex::encode([0x11_u8; 32]);

    let header1 = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock1 = state.block(header1);
    let mut stx1 = sblock1.transaction();
    stx1.world.governance_referenda_mut().insert(
        rid.clone(),
        GovernanceReferendumRecord {
            h_start: 1,
            h_end: 2,
            status: GovernanceReferendumStatus::Open,
            mode: GovernanceReferendumMode::Zk,
        },
    );
    stx1.world.elections_mut().insert(
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
            domain_tag: "gov:ballot:v1".to_string(),
        },
    );
    stx1.apply();

    let header2 = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut sblock2 = state.block(header2);
    let events = sblock2.world.take_external_events();
    let has_closed = events.iter().any(|event| {
        matches!(
            event.as_data_event(),
            Some(DataEvent::Governance(GovernanceEvent::ReferendumClosed(_)))
        )
    });
    assert!(has_closed, "expected ReferendumClosed at h_end");

    let has_decision = events.iter().any(|event| {
        matches!(
            event.as_data_event(),
            Some(DataEvent::Governance(
                GovernanceEvent::ProposalApproved(_) | GovernanceEvent::ProposalRejected(_)
            ))
        )
    });
    assert!(
        !has_decision,
        "unexpected decision without finalized zk tally"
    );
}
