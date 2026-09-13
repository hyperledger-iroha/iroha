// Included in network::tests; these invoke the real actor admission methods.
#[test]
fn authenticated_reader_requires_registered_connection_and_exact_outbound_peer() {
    let mut network = bare_network().expect("actor fixture resources");
    let peer = test_peer(socket_addr!(127.0.0.1:12801));
    let other = test_peer(socket_addr!(127.0.0.1:12802));
    for registered in [false, true] {
        if registered {
            network.connecting_peers.insert(801, other.clone());
        }
        let (cancel, cancelled) = watch::channel(false);
        let (reply, mut permission) = oneshot::channel();
        network.handle_service_message(ServiceMessage::Authenticated(Authenticated {
            peer: peer.clone(),
            connection_id: 801,
            session: [1; 32],
            relay_role: RelayRole::Hub,
            cancel,
            reply,
        }));
        assert!(*cancelled.borrow());
        assert!(matches!(
            permission.try_recv(),
            Err(oneshot::error::TryRecvError::Closed)
        ));
        assert!(!network.relay_trusted_peers.contains(peer.id()));
        assert!(network.peers.is_empty());
    }
}

#[test]
fn buffered_connected_after_physical_release_never_publishes_online_or_dispatch() {
    let mut network = bare_network().expect("actor fixture resources");
    let peer = test_peer(socket_addr!(127.0.0.1:12803));
    replace_test_authenticated_source_geometry(
        &mut network,
        2,
        Some(HashSet::from([peer.id().clone()])),
    );
    network.current_topology.insert(peer.id().clone());
    reserve_test_incoming(&mut network, 803);
    let (handle, receivers) = test_wire_peer_handle::<DummyMsg>(1);
    let (reply, mut permission) = oneshot::channel();
    network.handle_service_message(ServiceMessage::Authenticated(Authenticated {
        peer: peer.clone(),
        connection_id: 803,
        session: [0; 32],
        relay_role: RelayRole::Disabled,
        cancel: handle.termination_sender_for_test(),
        reply,
    }));
    let permit = permission.try_recv().unwrap();
    drop(permit);
    // Keep the actor release notification undrained, and the dispatch receiver
    // live, so neither mailbox closure nor a prior reap can mask this check.
    let (peer_message_sender, mut peer_message_receiver) = oneshot::channel();
    network.handle_service_message(ServiceMessage::Connected(Connected {
        peer: peer.clone(),
        connection_id: 803,
        disambiguator: 0,
        ready_peer_handle: handle,
        peer_message_sender,
        delivery_drain: InboundDeliveryDrain::completed_for_test(),
        relay_role: RelayRole::Disabled,
        scion_supported: false,
        trust_gossip: true,
    }));
    assert!(network.peers.is_empty());
    assert!(network.reply_route_tenures.is_empty());
    assert!(receivers.termination_requested());
    assert!(matches!(
        peer_message_receiver.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    ));
    assert!(network.terminating_connections.contains(&803));
}

#[test]
fn configured_hub_identity_proof_survives_losing_outbound_session_without_rank_override() {
    let mut network = bare_network().expect("actor fixture resources");
    network.relay_mode = iroha_config::parameters::actual::RelayMode::Spoke;
    let hub = test_peer(socket_addr!(127.0.0.1:12804));
    network.relay_hub_addresses.push(hub.address().clone());
    reserve_test_incoming(&mut network, 804);
    let (cancel, current_cancelled) = watch::channel(false);
    let (reply, mut permission) = oneshot::channel();
    network.peer_authenticated(Authenticated {
        peer: hub.clone(),
        connection_id: 804,
        session: [2; 32],
        relay_role: RelayRole::Hub,
        cancel,
        reply,
    });
    let _held = permission.try_recv().unwrap();
    assert!(!network.relay_trusted_peers.contains(hub.id()));
    network.connecting_peers.insert(805, hub.clone());
    network.outbound_connections.insert(805);
    let (cancel, refused) = watch::channel(false);
    let (reply, mut permission) = oneshot::channel();
    network.peer_authenticated(Authenticated {
        peer: hub.clone(),
        connection_id: 805,
        session: [1; 32],
        relay_role: RelayRole::Hub,
        cancel,
        reply,
    });
    assert!(network.relay_trusted_peers.contains(hub.id()));
    assert!(*refused.borrow() && !*current_cancelled.borrow());
    assert!(matches!(
        permission.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    ));
    assert!(
        network.peers.is_empty(),
        "identity proof does not publish an application session"
    );
    assert!(
        network
            .reader_arbitration
            .claim_connected(804, hub.id(), u64::from_be_bytes([2; 8]))
    );
}
