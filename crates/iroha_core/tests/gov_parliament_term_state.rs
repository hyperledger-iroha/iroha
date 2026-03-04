//! `ParliamentTerm` state shape and defaults.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "bls")]

use iroha_core::governance::state::ParliamentTerm;
use iroha_data_model::isi::governance::CouncilDerivationKind;
use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID};

#[test]
fn parliament_term_defaults_and_fields() {
    let term = ParliamentTerm::default();
    assert_eq!(term.epoch, 0);
    assert!(term.members.is_empty());
    assert!(term.alternates.is_empty());
    assert_eq!(term.verified, 0);
    assert_eq!(term.candidate_count, 0);
    assert_eq!(term.derived_by, CouncilDerivationKind::Fallback);
}

#[test]
fn parliament_term_roundtrips() {
    let members = vec![ALICE_ID.clone(), BOB_ID.clone()];
    let alternates = vec![CARPENTER_ID.clone()];
    let term = ParliamentTerm {
        epoch: 5,
        members: members.clone(),
        alternates: alternates.clone(),
        verified: 3,
        candidate_count: 4,
        derived_by: CouncilDerivationKind::Vrf,
    };
    let value = norito::json::to_value(&term).expect("json");
    let json = norito::json::to_string(&value).expect("json");
    let back_value: norito::json::Value = norito::json::from_str(&json).expect("parse");
    let back: ParliamentTerm = norito::json::from_value(back_value).expect("decode term");
    assert_eq!(back.epoch, term.epoch);
    assert_eq!(back.members, members);
    assert_eq!(back.alternates, alternates);
    assert_eq!(back.verified, 3);
    assert_eq!(back.candidate_count, 4);
    assert_eq!(back.derived_by, CouncilDerivationKind::Vrf);
}

#[test]
fn parliament_term_replaces_member_with_alternate() {
    let mut term = ParliamentTerm {
        epoch: 1,
        members: vec![ALICE_ID.clone(), BOB_ID.clone()],
        alternates: vec![CARPENTER_ID.clone()],
        verified: 2,
        candidate_count: 3,
        derived_by: CouncilDerivationKind::Vrf,
    };
    assert!(term.replace_member(&ALICE_ID));
    assert_eq!(term.members, vec![CARPENTER_ID.clone(), BOB_ID.clone()]);
    assert!(term.alternates.is_empty());
    assert_eq!(term.candidate_count, 3);
    assert_eq!(term.verified, 2);
}
