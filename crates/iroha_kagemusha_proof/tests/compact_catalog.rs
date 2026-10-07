//! Actual compact catalog closure. Every signed source chain is rebuilt under
//! the common outer key, with exact descriptor/key equality across the roundtrip.
//! Two/three terminal component catalogs are not the complete release catalog.

/// Shared real controls-off Send and its authenticated predecessor builders.
#[path = "a_send_recursive.rs"]
pub mod send_chain;

use send_chain::load_outer;

include!("common/proof_fixtures/compact_catalog_body.rs");

fn send_terminal(source: &send_chain::AuthenticatedSend) -> Terminal<'_> {
    Terminal {
        key: &source.key,
        binding: &source.binding,
        proof: &source.proof,
        instances: &source.instances,
        part: &source.vesta_part,
        opening: &source.opening,
        predecessor: &source.predecessor_vesta,
    }
}

fn catalog_roundtrip(include_send: bool) {
    let initial_root = bootstrap_outer::compact_bootstrap::rooted_compact_bootstrap();
    let initial_load =
        load_chain::authenticated_load_with_profile(&initial_root, PROFILE, Some(2), false);
    let bootstrap = &initial_root.source;
    assert_eq!(
        bootstrap.binding, initial_load.binding,
        "uniform actual terminal A descriptor"
    );
    let trivial = AccumulatorT::trivial(
        &PinnedParams::<Eq>::derive(16).unwrap(),
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let catalog = vec![bootstrap.key.clone(), initial_load.key.clone()];
    let program = Program::new(
        bootstrap_terminal(bootstrap, &trivial),
        catalog,
        &initial_root.binding,
    );
    let load = rebuild_bootstrap_load(&program, bootstrap, &initial_load);
    eprintln!(
        "COMPACT_TWO_TERMINAL_CLOSURE source_keys_equal=true common_key_bound_before_signing=true all_native_proofs=true transport=4800 full_catalog=false"
    );
    if !include_send {
        return;
    }

    let initial_send = send_chain::run_send_from_load(&load, PROFILE, Some(2));
    assert_eq!(
        initial_send.binding, bootstrap.binding,
        "Send must share the exact admitted A descriptor"
    );
    let mut catalog = program.catalog.clone();
    catalog.push(initial_send.key.clone());
    let program = Program::new(
        bootstrap_terminal(bootstrap, &trivial),
        catalog,
        &initial_root.binding,
    );
    let load = rebuild_bootstrap_load(&program, bootstrap, &initial_load);
    let send = send_chain::run_send_from_load(&load, PROFILE, Some(2));
    assert_eq!(send.binding, initial_send.binding);
    assert_eq!(send.key.to_bytes(), initial_send.key.to_bytes());
    assert_eq!(send.state.lineage[17], program.digest());
    let _outer = program.prove(send_terminal(&send), "Send-mask0");
    send.pallas
        .decide(
            &PinnedParams::<Ep>::derive(16).unwrap(),
            MemoryBudget::DEFAULT,
        )
        .unwrap();
    eprintln!(
        "COMPACT_THREE_TERMINAL_CLOSURE source_keys_equal=true common_key_bound_before_signing=true all_native_proofs=true transport=4800 mask0_only=true full_catalog=false"
    );
}

#[test]
#[ignore = "actual compact Bootstrap/Load chains rebuilt under one immutable two-terminal key"]
fn compact_bootstrap_load_catalog_rebinds_every_proof_and_key() {
    catalog_roundtrip(false);
}

#[test]
#[ignore = "actual compact Bootstrap/Load/Send-mask0 rebuilt under one immutable three-terminal key"]
fn compact_bootstrap_load_send_catalog_rebinds_every_proof_and_key() {
    catalog_roundtrip(true);
}

#[test]
#[ignore = "genuine distinct payer Load and receiver Bootstrap rebuilt under one compact key"]
fn compact_distinct_wallets_share_the_exact_predecessor_catalog() {
    let wallets = compact_payer_load_and_receiver();
    assert_eq!(wallets.payer.binding, wallets.receiver.binding);
    assert_eq!(
        wallets.payer.key.to_bytes(),
        wallets.receiver.key.to_bytes()
    );
    assert_eq!(wallets.payer.proof.len(), 3712);
    assert_eq!(wallets.receiver.proof.len(), 3712);
    assert_ne!(
        wallets.payer.source.state.core[5..7],
        wallets.receiver.source.state.core[5..7]
    );
}
