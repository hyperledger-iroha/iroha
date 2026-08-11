// Exact-network regressions for transaction-gossip target selection.

#[test]
fn initial_target_seed_is_stable_and_exact_network_specific() {
    let network_id = test_network_id();
    let foreign_network_id: NetworkId =
        "0000000000000000000000000000000000000000000000000000000000000002"
            .parse()
            .expect("valid foreign test network id");
    let self_peer: PeerId = (*PEER_KEYPAIR).public_key().clone().into();
    let other_peer: PeerId = (*BOB_KEYPAIR).public_key().clone().into();
    let max_peer: PeerId = (*CARPENTER_KEYPAIR).public_key().clone().into();

    let seed = TransactionGossiper::initial_target_seed(
        &network_id,
        &self_peer,
        &max_peer,
        GOSSIP_SEED_PUBLIC_DOMAIN,
    );
    assert_eq!(
        seed,
        TransactionGossiper::initial_target_seed(
            &network_id,
            &self_peer,
            &max_peer,
            GOSSIP_SEED_PUBLIC_DOMAIN,
        ),
        "initial target seed should be stable for the same identity inputs"
    );
    assert_ne!(
        seed,
        TransactionGossiper::initial_target_seed(
            &network_id,
            &self_peer,
            &max_peer,
            GOSSIP_SEED_RESTRICTED_DOMAIN,
        ),
        "public and restricted gossip planes should start from distinct seeds"
    );
    assert_ne!(
        seed,
        TransactionGossiper::initial_target_seed(
            &network_id,
            &other_peer,
            &max_peer,
            GOSSIP_SEED_PUBLIC_DOMAIN,
        ),
        "local peer identity should perturb initial target seed"
    );
    assert_ne!(
        seed,
        TransactionGossiper::initial_target_seed(
            &foreign_network_id,
            &self_peer,
            &max_peer,
            GOSSIP_SEED_PUBLIC_DOMAIN,
        ),
        "different genesis lineages must perturb the initial gossip target seed"
    );
}
