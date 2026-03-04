//! Replacement flow for governance VRF draw alternates.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![cfg(feature = "bls")]

use iroha_core::governance::draw::replace_with_alternate;
use iroha_test_samples::gen_account_in;

#[test]
fn replace_member_with_alternate() {
    let (a, _) = gen_account_in("wonderland");
    let (b, _) = gen_account_in("wonderland");
    let (c, _) = gen_account_in("wonderland");
    let (d, _) = gen_account_in("wonderland");

    let mut members = vec![a.clone(), b.clone()];
    let mut alternates = vec![c.clone(), d.clone()];

    // Replace bob with carol; bob leaves, carol joins, dave remains alternate.
    assert!(replace_with_alternate(
        members.as_mut_slice(),
        &mut alternates,
        &b
    ));
    assert_eq!(members, vec![a.clone(), c.clone()]);
    assert_eq!(alternates, vec![d.clone()]);

    // Missing member not found → no replacement.
    assert!(!replace_with_alternate(
        members.as_mut_slice(),
        &mut alternates,
        &b
    ));
    assert_eq!(members, vec![a, c]);
    assert_eq!(alternates, vec![d]);
}
