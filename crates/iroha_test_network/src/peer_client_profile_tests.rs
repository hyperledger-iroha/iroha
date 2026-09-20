#[test]
fn peer_clients_preserve_selected_network_profile_after_builder_scope() {
    use iroha_data_model::account::address::{ChainDiscriminantGuard, chain_discriminant};
    use iroha_test_samples::BOB_KEYPAIR;

    for selected in [
        None,
        Some(("fc56984b-2be7-431d-840e-21514d1883f0", 369_u16)),
    ] {
        let (expected_chain, expected_discriminant) = selected.map_or_else(
            || (config::chain_id(), defaults::common::chain_discriminant()),
            |(chain, discriminant)| (ChainId::from(chain), discriminant),
        );
        // The builder's formatting scope ends before clients are created, as it does when
        // the async acceptance fixture returns from its blocking preparation thread.
        let network = {
            let _profile = ChainDiscriminantGuard::enter(expected_discriminant);
            let mut builder = NetworkBuilder::new().with_peers(4);
            if let Some((chain, discriminant)) = selected {
                builder = builder
                    .with_npos_consensus()
                    .with_config_layer(move |layer| {
                        layer
                            .write("chain", chain)
                            .write("chain_discriminant", i64::from(discriminant));
                    });
            }
            build_with_isolated_permit(builder)
        };
        let expected_network = network.network_id();
        assert_eq!(network.chain_id(), expected_chain);
        let _foreign = ChainDiscriminantGuard::enter(777);
        let assert_identity = |client: &AsyncClient, account: &AccountId, key: &KeyPair| {
            assert_eq!(client.chain(), &expected_chain);
            assert_eq!(*client.network_id(), expected_network);
            assert_eq!(client.account_chain_discriminant(), expected_discriminant);
            assert_eq!(client.account(), account);
            assert_eq!(client.key_pair(), key);
            assert_eq!(
                chain_discriminant(),
                777,
                "client construction leaked a profile scope"
            );
        };
        let first = network.client();
        assert_identity(first.client(), &ALICE_ID, &ALICE_KEYPAIR);
        for peer in network.all_peers() {
            let alice = peer.client();
            assert_identity(alice.client(), &ALICE_ID, &ALICE_KEYPAIR);
            let cloned = peer.clone();
            let bob = cloned.async_client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
            assert_identity(&bob, &BOB_ID, &BOB_KEYPAIR);
            assert_eq!(bob.operator_key_pair(), Some(&peer.key_pair));
            assert_eq!(bob.endpoint().as_str(), format!("{}/", peer.torii_url()));
            let rebuilt = bob
                .to_builder()
                .build()
                .expect("rebuild selected client context");
            assert_identity(&rebuilt, &BOB_ID, &BOB_KEYPAIR);
            assert_eq!(rebuilt.operator_key_pair(), bob.operator_key_pair());
            let blocking = peer.client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());
            assert_identity(blocking.client(), &BOB_ID, &BOB_KEYPAIR);
            assert_eq!(
                blocking.client().operator_key_pair(),
                bob.operator_key_pair()
            );
        }
    }
}
