//! Genuine source evidence for the current generic Ω/Payment size failure.
//! This deliberately exercises rejection, never an accepted Receive fixture.
#![allow(clippy::duplicate_mod)]
#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
mod common;
#[path = "common/load_objects.rs"]
#[allow(dead_code)]
mod load_objects;
#[path = "load_omega.rs"]
mod load_outer;
#[path = "a_send.rs"]
mod send_components;

#[test]
#[ignore = "genuine full Bootstrap/Load Ω and Send σ source proofs; expensive native source qualification"]
fn genuine_generic_omega_is_rejected_by_receive_payment_cap() {
    let rooted = load_outer::two_terminal_load_omega(false);
    let source = &rooted.artifact;
    let send = send_components::genuine_send_source(&source.source.state);
    let omega = 320
        + source.proof.len()
        + source.source.pallas.to_bytes().len()
        + source.vesta.to_bytes().len();
    let sigma = send.sigma.len();
    assert!(
        omega + sigma
            > iroha_kagemusha_proof::a_relation::native::receive::MAX_PAYMENT_ORIGINAL_BYTES
    );
    assert_eq!(
        iroha_kagemusha_proof::a_relation::native::receive::check_payment_original_sizes(
            omega, sigma
        ),
        Err(iroha_kagemusha_proof::a_relation::native::receive::Error::Input)
    );
}
