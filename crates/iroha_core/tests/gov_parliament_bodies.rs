//! Parliament body derivation should emit rosters for every governance body.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_config::parameters::actual::Governance;
use iroha_core::governance::{draw::derive_parliament_bodies, state::ParliamentTerm};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId, account::AccountId, governance::types::ParliamentBody,
    isi::governance::CouncilDerivationKind,
};

fn account(tag: u8) -> AccountId {
    let (public_key, _) = KeyPair::from_seed(vec![tag; 32], Algorithm::Ed25519).into_parts();
    AccountId::new(public_key)
}

#[test]
fn derive_bodies_populates_all_rosters() {
    let chain: ChainId = "demo-chain".into();
    let beacon = [0xAB; 32];
    let cfg = Governance {
        rules_committee_size: 2,
        agenda_council_size: 2,
        interest_panel_size: 2,
        review_panel_size: 2,
        policy_jury_size: 3,
        oversight_committee_size: 1,
        fma_committee_size: 1,
        parliament_alternate_size: Some(1),
        ..Governance::default()
    };

    let council = ParliamentTerm {
        epoch: 4,
        members: vec![account(b'A'), account(b'B'), account(b'C'), account(b'D')],
        alternates: vec![account(b'E')],
        verified: 3,
        candidate_count: 5,
        derived_by: CouncilDerivationKind::Vrf,
    };

    let bodies = derive_parliament_bodies(&cfg, &chain, council.epoch, &beacon, &council);
    assert_eq!(bodies.selection_epoch, council.epoch);

    for body in [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ] {
        let roster = bodies
            .rosters
            .get(&body)
            .unwrap_or_else(|| panic!("{body:?} roster missing"));
        assert_eq!(roster.epoch, council.epoch);
        assert_eq!(roster.verified, council.verified);
        assert!(!roster.members.is_empty(), "expected members for {body:?}");
    }

    let rules = bodies
        .rosters
        .get(&ParliamentBody::RulesCommittee)
        .expect("rules roster present");
    assert_eq!(rules.members.len(), 2);
    assert_eq!(rules.alternates.len(), 1);
}
