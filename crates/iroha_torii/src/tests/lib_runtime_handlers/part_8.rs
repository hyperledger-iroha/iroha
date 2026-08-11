
    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn incoming_read_proxy_rejects_inactive_autoscale_range_lane_hint() {
        let mut app = mk_app_state_for_tests();
        let (inactive_lane, inactive_dataspace) =
            torii_routed_read_tests::configure_corrupt_inactive_autoscale_range_route_for_test(
                &mut app,
            );
        let route = RoutingDecision::new(inactive_lane, inactive_dataspace);

        let response = incoming_read_proxy_response_for_route(app, route).await;

        assert_incoming_proxy_stale_route_rejection(&response, route);
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn incoming_read_proxy_rejects_future_created_autoscale_lane_hint() {
        let mut app = mk_app_state_for_tests();
        let (future_lane, future_dataspace) =
            torii_routed_read_tests::configure_future_created_autoscale_route_for_test(&mut app);
        let route = RoutingDecision::new(future_lane, future_dataspace);

        let response = incoming_read_proxy_response_for_route(app, route).await;

        assert_incoming_proxy_stale_route_rejection(&response, route);
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn incoming_verified_query_proxy_rejects_retired_lane_hint() {
        let app = mk_app_state_for_tests();
        let route = RoutingDecision::new(LaneId::new(43), DataSpaceId::UNIVERSAL);

        let response = incoming_verified_query_proxy_response_for_route(app, route).await;

        assert_incoming_proxy_stale_route_rejection(&response, route);
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn incoming_verified_query_proxy_rejects_lane_dataspace_mismatch_hint() {
        let mut app = mk_app_state_for_tests();
        configure_multiple_dataspace_routes_for_test(&mut app);
        let route = RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL);

        let response = incoming_verified_query_proxy_response_for_route(app, route).await;

        assert_incoming_proxy_stale_route_rejection(&response, route);
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn incoming_verified_query_proxy_rejects_inactive_autoscale_range_lane_hint() {
        let mut app = mk_app_state_for_tests();
        let (inactive_lane, inactive_dataspace) =
            torii_routed_read_tests::configure_corrupt_inactive_autoscale_range_route_for_test(
                &mut app,
            );
        let route = RoutingDecision::new(inactive_lane, inactive_dataspace);

        let response = incoming_verified_query_proxy_response_for_route(app, route).await;

        assert_incoming_proxy_stale_route_rejection(&response, route);
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn incoming_verified_query_proxy_rejects_future_created_autoscale_lane_hint() {
        let mut app = mk_app_state_for_tests();
        let (future_lane, future_dataspace) =
            torii_routed_read_tests::configure_future_created_autoscale_route_for_test(&mut app);
        let route = RoutingDecision::new(future_lane, future_dataspace);

        let response = incoming_verified_query_proxy_response_for_route(app, route).await;

        assert_incoming_proxy_stale_route_rejection(&response, route);
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn torii_proxy_candidate_peers_only_use_authoritative_peers() {
        let local_keypair =
            checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
        let authoritative_validator_keypair = checked_torii_test_ed25519_keypair(
            0x5e,
            "derive authoritative-lane manifest validator fixture key",
        );
        let authoritative_keypair = checked_torii_test_keypair_from_seed_byte(
            0x5f,
            Algorithm::BlsNormal,
            "derive authoritative-lane manifest peer fixture key",
        );
        let fallback_keypair = checked_torii_test_ed25519_keypair(
            0x60,
            "derive authoritative-lane fallback peer fixture key",
        );
        let local_peer_id = PeerId::from(local_keypair.public_key().clone());
        let authoritative_validator =
            AccountId::new(authoritative_validator_keypair.public_key().clone());
        let authoritative_peer_id = PeerId::from(authoritative_keypair.public_key().clone());
        let fallback_peer_id = PeerId::from(fallback_keypair.public_key().clone());

        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let (online_tx, online_rx) =
                tokio::sync::watch::channel(std::collections::HashSet::new());
            let local_peer = Peer::new(
                "127.0.0.1:10001".parse().expect("valid local address"),
                local_keypair.public_key().clone(),
            );
            let authoritative_peer = Peer::new(
                "127.0.0.1:10002"
                    .parse()
                    .expect("valid authoritative address"),
                authoritative_keypair.public_key().clone(),
            );
            let fallback_peer = Peer::new(
                "127.0.0.1:10003".parse().expect("valid fallback address"),
                fallback_keypair.public_key().clone(),
            );
            online_tx
                .send(std::collections::HashSet::from([
                    local_peer,
                    authoritative_peer,
                    fallback_peer,
                ]))
                .expect("online peers update should succeed");
            app_mut.online_peers = OnlinePeersProvider::new(online_rx);
            app_mut.local_peer_id = Some(local_peer_id.clone());
        }

        {
            let mut topology = app.state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.push(authoritative_peer_id.clone());
            topology.commit();
        }
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
            ensure_runtime_peer_binding_for_test(
                state,
                &authoritative_validator,
                &authoritative_keypair,
                "authoritative",
            );
            install_lane_manifest_registry_for_test(
                state,
                &[(
                    LaneId::SINGLE,
                    vec![(authoritative_validator, authoritative_peer_id.clone())],
                )],
            );
        }

        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let authoritative_without_local: Vec<_> =
            super::authoritative_lane_peers(app.as_ref(), route)
                .authoritative
                .into_iter()
                .filter(|peer_id| peer_id != &local_peer_id)
                .map(ToriiProxyCandidate::P2p)
                .collect();
        let candidates =
            super::torii_proxy_candidate_peer_ids(app.as_ref(), &local_peer_id, route, None, &[]);

        assert_eq!(
            candidates.authoritative_count,
            authoritative_without_local.len()
        );
        assert_eq!(candidates.peers, authoritative_without_local);
        assert_eq!(candidates.authoritative_total_count, 1);
        assert_eq!(candidates.offline_authoritative_count, 0);
        assert_eq!(candidates.bridge_authoritative_count, 0);
        assert!(candidates.unavailable_reason.is_none());
        assert!(
            !candidates
                .peers
                .iter()
                .any(|candidate| candidate.peer_id() == &fallback_peer_id)
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn torii_proxy_candidate_peers_reject_future_created_autoscale_manifest_authority() {
        let local_keypair = checked_torii_test_ed25519_keypair(
            0x61,
            "derive future-created candidate local fixture key",
        );
        let authoritative_validator_keypair = checked_torii_test_ed25519_keypair(
            0x62,
            "derive future-created candidate manifest validator fixture key",
        );
        let authoritative_keypair = checked_torii_test_keypair_from_seed_byte(
            0x63,
            Algorithm::BlsNormal,
            "derive future-created candidate manifest peer fixture key",
        );
        let local_peer_id = PeerId::from(local_keypair.public_key().clone());
        let authoritative_validator =
            AccountId::new(authoritative_validator_keypair.public_key().clone());
        let authoritative_peer_id = PeerId::from(authoritative_keypair.public_key().clone());

        let mut app = mk_app_state_for_tests();
        let (future_lane, future_dataspace) =
            torii_routed_read_tests::configure_future_created_autoscale_route_for_test(&mut app);
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let (online_tx, online_rx) =
                tokio::sync::watch::channel(std::collections::HashSet::new());
            online_tx
                .send(std::collections::HashSet::from([
                    Peer::new(
                        "127.0.0.1:10001".parse().expect("valid local address"),
                        local_keypair.public_key().clone(),
                    ),
                    Peer::new(
                        "127.0.0.1:10002"
                            .parse()
                            .expect("valid authoritative address"),
                        authoritative_keypair.public_key().clone(),
                    ),
                ]))
                .expect("online peers update should succeed");
            app_mut.online_peers = OnlinePeersProvider::new(online_rx);
            app_mut.local_peer_id = Some(local_peer_id.clone());

            let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
            ensure_runtime_peer_binding_for_test(
                state,
                &authoritative_validator,
                &authoritative_keypair,
                "authoritative",
            );
            {
                let mut topology = state.commit_topology.block();
                topology.clear();
                topology.push(local_peer_id.clone());
                topology.push(authoritative_peer_id.clone());
                topology.commit();
            }
            install_lane_manifest_registry_for_test(
                state,
                &[(
                    future_lane,
                    vec![(authoritative_validator, authoritative_peer_id.clone())],
                )],
            );
            assert!(
                !state.is_lane_active_for_authority(future_lane),
                "fixture lane should remain inactive after manifest binding setup"
            );
        }

        let route = RoutingDecision::new(future_lane, future_dataspace);
        assert!(
            super::authoritative_lane_peers(app.as_ref(), route)
                .authoritative
                .is_empty(),
            "future-created autoscale manifest bindings must not become proxy authority early"
        );

        let candidates =
            super::torii_proxy_candidate_peer_ids(app.as_ref(), &local_peer_id, route, None, &[]);

        assert_eq!(candidates.authoritative_count, 0);
        assert_eq!(candidates.authoritative_total_count, 0);
        assert_eq!(candidates.offline_authoritative_count, 0);
        assert_eq!(candidates.bridge_authoritative_count, 0);
        assert!(candidates.peers.is_empty());
        assert_eq!(
            candidates.unavailable_reason,
            Some(ToriiProxyUnavailableReason::MissingAuthoritativeBinding)
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn torii_proxy_candidate_peers_fail_closed_when_manifest_authoritative_peers_are_offline()
    {
        let local_keypair =
            checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
        let authoritative_validator_keypair = checked_torii_test_ed25519_keypair(
            0x5e,
            "derive authoritative-lane manifest validator fixture key",
        );
        let authoritative_keypair = checked_torii_test_keypair_from_seed_byte(
            0x5f,
            Algorithm::BlsNormal,
            "derive authoritative-lane manifest peer fixture key",
        );
        let local_peer_id = PeerId::from(local_keypair.public_key().clone());
        let authoritative_validator =
            AccountId::new(authoritative_validator_keypair.public_key().clone());
        let authoritative_peer_id = PeerId::from(authoritative_keypair.public_key().clone());

        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let (online_tx, online_rx) =
                tokio::sync::watch::channel(std::collections::HashSet::new());
            online_tx
                .send(std::collections::HashSet::from([Peer::new(
                    "127.0.0.1:10001".parse().expect("valid local address"),
                    local_keypair.public_key().clone(),
                )]))
                .expect("online peers update should succeed");
            app_mut.online_peers = OnlinePeersProvider::new(online_rx);
            app_mut.local_peer_id = Some(local_peer_id.clone());
        }

        {
            let mut topology = app.state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.push(authoritative_peer_id.clone());
            topology.commit();
        }
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
            ensure_runtime_peer_binding_for_test(
                state,
                &authoritative_validator,
                &authoritative_keypair,
                "authoritative",
            );
            install_lane_manifest_registry_for_test(
                state,
                &[(
                    LaneId::SINGLE,
                    vec![(authoritative_validator, authoritative_peer_id.clone())],
                )],
            );
        }

        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let candidates =
            super::torii_proxy_candidate_peer_ids(app.as_ref(), &local_peer_id, route, None, &[]);

        assert_eq!(candidates.authoritative_count, 0);
        assert_eq!(candidates.authoritative_total_count, 1);
        assert_eq!(candidates.offline_authoritative_count, 1);
        assert_eq!(candidates.bridge_authoritative_count, 0);
        assert!(candidates.peers.is_empty());
        assert_eq!(
            candidates.unavailable_reason,
            Some(ToriiProxyUnavailableReason::AuthoritativePeersOffline)
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn torii_proxy_candidate_peers_bridge_to_offline_manifest_authority_when_torii_url_is_present()
     {
        let local_keypair =
            checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
        let authoritative_validator_keypair = checked_torii_test_ed25519_keypair(
            0x5e,
            "derive authoritative-lane manifest validator fixture key",
        );
        let authoritative_keypair = checked_torii_test_keypair_from_seed_byte(
            0x5f,
            Algorithm::BlsNormal,
            "derive authoritative-lane manifest peer fixture key",
        );
        let local_peer_id = PeerId::from(local_keypair.public_key().clone());
        let authoritative_validator =
            AccountId::new(authoritative_validator_keypair.public_key().clone());
        let authoritative_peer_id = PeerId::from(authoritative_keypair.public_key().clone());

        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let (online_tx, online_rx) =
                tokio::sync::watch::channel(std::collections::HashSet::new());
            online_tx
                .send(std::collections::HashSet::from([Peer::new(
                    "127.0.0.1:10001".parse().expect("valid local address"),
                    local_keypair.public_key().clone(),
                )]))
                .expect("online peers update should succeed");
            app_mut.online_peers = OnlinePeersProvider::new(online_rx);
            app_mut.local_peer_id = Some(local_peer_id.clone());
        }

        {
            let mut topology = app.state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.push(authoritative_peer_id.clone());
            topology.commit();
        }
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
            ensure_runtime_peer_binding_for_test(
                state,
                &authoritative_validator,
                &authoritative_keypair,
                "authoritative",
            );
            install_lane_manifest_registry_with_torii_urls_for_test(
                state,
                &[(
                    LaneId::SINGLE,
                    vec![(
                        authoritative_validator,
                        authoritative_peer_id.clone(),
                        Some("http://127.0.0.1:19080"),
                    )],
                )],
            );
        }

        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let candidates =
            super::torii_proxy_candidate_peer_ids(app.as_ref(), &local_peer_id, route, None, &[]);

        assert_eq!(candidates.authoritative_count, 1);
        assert_eq!(candidates.authoritative_total_count, 1);
        assert_eq!(candidates.offline_authoritative_count, 1);
        assert_eq!(candidates.bridge_authoritative_count, 1);
        assert_eq!(
            candidates.peers,
            vec![ToriiProxyCandidate::HttpBridge {
                peer_id: authoritative_peer_id.clone(),
                torii_url: "http://127.0.0.1:19080".to_owned(),
            }]
        );
        assert!(candidates.unavailable_reason.is_none());

        let proposal_height = app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let exact_candidates = super::queue_plan_synced_proxy_candidate_peer_ids(
            app.as_ref(),
            &local_peer_id,
            route,
            std::slice::from_ref(&authoritative_peer_id),
            proposal_height,
            None,
            &[],
            false,
        );
        assert_eq!(
            exact_candidates.peers,
            vec![ToriiProxyCandidate::HttpBridge {
                peer_id: authoritative_peer_id,
                torii_url: "http://127.0.0.1:19080".to_owned(),
            }],
            "proposal-bound exact authorities must retain their manifest HTTP bridge"
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn execute_torii_proxy_request_with_fallback_returns_route_unavailable_when_manifest_authoritative_peers_are_offline()
     {
        let local_keypair =
            checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
        let authoritative_validator_keypair = checked_torii_test_ed25519_keypair(
            0x5e,
            "derive authoritative-lane manifest validator fixture key",
        );
        let authoritative_keypair = checked_torii_test_keypair_from_seed_byte(
            0x5f,
            Algorithm::BlsNormal,
            "derive authoritative-lane manifest peer fixture key",
        );
        let local_peer_id = PeerId::from(local_keypair.public_key().clone());
        let authoritative_validator =
            AccountId::new(authoritative_validator_keypair.public_key().clone());
        let authoritative_peer_id = PeerId::from(authoritative_keypair.public_key().clone());

        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let (online_tx, online_rx) =
                tokio::sync::watch::channel(std::collections::HashSet::new());
            online_tx
                .send(std::collections::HashSet::from([Peer::new(
                    "127.0.0.1:10001".parse().expect("valid local address"),
                    local_keypair.public_key().clone(),
                )]))
                .expect("online peers update should succeed");
            app_mut.online_peers = OnlinePeersProvider::new(online_rx);
            app_mut.local_peer_id = Some(local_peer_id.clone());
        }

        {
            let mut topology = app.state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.push(authoritative_peer_id.clone());
            topology.commit();
        }
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
            ensure_runtime_peer_binding_for_test(
                state,
                &authoritative_validator,
                &authoritative_keypair,
                "authoritative",
            );
            install_lane_manifest_registry_for_test(
                state,
                &[(
                    LaneId::SINGLE,
                    vec![(authoritative_validator, authoritative_peer_id.clone())],
                )],
            );
        }

        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let response = super::execute_torii_proxy_request_with_fallback(
            &app,
            route,
            ToriiProxyRequestKindV4::SignedQueryRouteScan {
                query_bytes: Vec::new(),
                expected_route: ToriiRouteHintV1::from(route),
                response_format: ToriiProxyResponseFormatV1::Norito,
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect error body");
        let envelope = norito::decode_from_bytes::<super::ErrorEnvelope>(&body)
            .expect("decode error envelope");
        assert_eq!(envelope.code(), "route_unavailable");
        assert_eq!(
            envelope.message(),
            ToriiProxyUnavailableReason::AuthoritativePeersOffline.ingress_message(route)
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn torii_proxy_candidate_peers_fail_closed_when_bindings_are_missing() {
        let local_keypair =
            checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
        let fallback_keypair = checked_torii_test_ed25519_keypair(
            0x60,
            "derive authoritative-lane fallback peer fixture key",
        );
        let local_peer_id = PeerId::from(local_keypair.public_key().clone());
        let fallback_peer_id = PeerId::from(fallback_keypair.public_key().clone());

        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let (online_tx, online_rx) =
                tokio::sync::watch::channel(std::collections::HashSet::new());
            let local_peer = Peer::new(
                "127.0.0.1:10001".parse().expect("valid local address"),
                local_keypair.public_key().clone(),
            );
            let fallback_peer = Peer::new(
                "127.0.0.1:10002".parse().expect("valid fallback address"),
                fallback_keypair.public_key().clone(),
            );
            online_tx
                .send(std::collections::HashSet::from([local_peer, fallback_peer]))
                .expect("online peers update should succeed");
            app_mut.online_peers = OnlinePeersProvider::new(online_rx);
            app_mut.local_peer_id = Some(local_peer_id.clone());
        }

        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        block
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(
                SumeragiNposParameters::default().into_custom_parameter(),
            ));
        let mut peers = block.world.peers_mut_for_testing().transaction();
        peers.clear();
        peers.push(local_peer_id.clone());
        peers.push(fallback_peer_id.clone());
        peers.apply();
        block.commit().expect("commit npos peer roster");

        let route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
        let candidates =
            super::torii_proxy_candidate_peer_ids(app.as_ref(), &local_peer_id, route, None, &[]);

        assert!(
            super::authoritative_lane_peers(app.as_ref(), route)
                .authoritative
                .is_empty(),
            "non-core route should still have no local authoritative view"
        );
        assert_eq!(candidates.authoritative_count, 0);
        assert_eq!(candidates.authoritative_total_count, 0);
        assert_eq!(candidates.offline_authoritative_count, 0);
        assert_eq!(candidates.bridge_authoritative_count, 0);
        assert!(candidates.peers.is_empty());
        assert_eq!(
            candidates.unavailable_reason,
            Some(ToriiProxyUnavailableReason::MissingAuthoritativeBinding)
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn torii_proxy_candidates_exclude_self_sender_visited_and_fail_closed_when_exhausted() {
        let local_keypair =
            checked_torii_test_ed25519_keypair(0x58, "derive authoritative-lane local fixture key");
        let authoritative_validator_keypair = checked_torii_test_ed25519_keypair(
            0x5e,
            "derive authoritative-lane manifest validator fixture key",
        );
        let sender_validator_keypair = checked_torii_test_ed25519_keypair(
            0x63,
            "derive authoritative-lane sender validator fixture key",
        );
        let visited_validator_keypair = checked_torii_test_ed25519_keypair(
            0x65,
            "derive authoritative-lane visited validator fixture key",
        );
        let authoritative_keypair = checked_torii_test_keypair_from_seed_byte(
            0x5f,
            Algorithm::BlsNormal,
            "derive authoritative-lane manifest peer fixture key",
        );
        let sender_keypair = checked_torii_test_keypair_from_seed_byte(
            0x64,
            Algorithm::BlsNormal,
            "derive authoritative-lane sender peer fixture key",
        );
        let visited_keypair = checked_torii_test_keypair_from_seed_byte(
            0x66,
            Algorithm::BlsNormal,
            "derive authoritative-lane visited peer fixture key",
        );
        let local_peer_id = PeerId::from(local_keypair.public_key().clone());
        let authoritative_validator =
            AccountId::new(authoritative_validator_keypair.public_key().clone());
        let sender_validator = AccountId::new(sender_validator_keypair.public_key().clone());
        let visited_validator = AccountId::new(visited_validator_keypair.public_key().clone());
        let authoritative_peer_id = PeerId::from(authoritative_keypair.public_key().clone());
        let sender_peer_id = PeerId::from(sender_keypair.public_key().clone());
        let visited_peer_id = PeerId::from(visited_keypair.public_key().clone());

        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let (online_tx, online_rx) =
                tokio::sync::watch::channel(std::collections::HashSet::new());
            online_tx
                .send(std::collections::HashSet::from([
                    Peer::new(
                        "127.0.0.1:10001".parse().expect("valid local address"),
                        local_keypair.public_key().clone(),
                    ),
                    Peer::new(
                        "127.0.0.1:10002"
                            .parse()
                            .expect("valid authoritative address"),
                        authoritative_keypair.public_key().clone(),
                    ),
                    Peer::new(
                        "127.0.0.1:10003".parse().expect("valid sender address"),
                        sender_keypair.public_key().clone(),
                    ),
                    Peer::new(
                        "127.0.0.1:10004".parse().expect("valid visited address"),
                        visited_keypair.public_key().clone(),
                    ),
                ]))
                .expect("online peers update should succeed");
            app_mut.online_peers = OnlinePeersProvider::new(online_rx);
            app_mut.local_peer_id = Some(local_peer_id.clone());
        }

        {
            let mut topology = app.state.commit_topology.block();
            topology.clear();
            topology.push(authoritative_peer_id.clone());
            topology.commit();
        }
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
            ensure_runtime_peer_binding_for_test(
                state,
                &authoritative_validator,
                &authoritative_keypair,
                "authoritative",
            );
            ensure_runtime_peer_binding_for_test(
                state,
                &sender_validator,
                &sender_keypair,
                "sender",
            );
            ensure_runtime_peer_binding_for_test(
                state,
                &visited_validator,
                &visited_keypair,
                "visited",
            );
            install_lane_manifest_registry_for_test(
                state,
                &[(
                    LaneId::SINGLE,
                    vec![
                        (authoritative_validator, authoritative_peer_id.clone()),
                        (sender_validator, sender_peer_id.clone()),
                        (visited_validator, visited_peer_id.clone()),
                    ],
                )],
            );
        }

        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let candidates = super::torii_proxy_candidate_peer_ids(
            app.as_ref(),
            &local_peer_id,
            route,
            Some(&sender_peer_id),
            std::slice::from_ref(&visited_peer_id),
        );

        assert_eq!(candidates.authoritative_count, 1);
        assert_eq!(
            candidates.peers,
            vec![ToriiProxyCandidate::P2p(authoritative_peer_id.clone())]
        );
        assert!(
            !candidates
                .peers
                .iter()
                .any(|candidate| candidate.peer_id() == &sender_peer_id)
        );
        assert!(
            !candidates
                .peers
                .iter()
                .any(|candidate| candidate.peer_id() == &visited_peer_id)
        );
        assert_eq!(candidates.loop_prevention_drops, 2);

        let local_fanout_request = ToriiProxyRequestV5 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V5,
            request_id: Hash::new(b"local-nexus-fanout-cannot-target-self"),
            hop_count: 1,
            max_hops: TORII_PROXY_DEFAULT_MAX_HOPS,
            visited_peer_ids: vec![local_peer_id.clone()],
            request: ToriiProxyRequestKindV4::ReadFanout(super::torii_read_fanout_request(
                ToriiReadEndpointV1::AccountGet,
                ToriiFanoutRouteScopeV1::AllDataspaces,
                ToriiReadFanoutMergeV1::Account,
                vec![ALICE_ID.to_string()],
                None,
                Vec::new(),
                ToriiProxyResponseFormatV1::Json,
            )),
        };
        let fanout_candidates = super::torii_proxy_candidate_peer_ids_for_request(
            app.as_ref(),
            &local_peer_id,
            route,
            None,
            &local_fanout_request.visited_peer_ids,
            &local_fanout_request,
            true,
        )
        .expect("ordinary read fanout candidate selection");
        assert!(
            fanout_candidates.peers.iter().all(|candidate| {
                !matches!(candidate, ToriiProxyCandidate::Local(_))
                    && candidate.peer_id() != &local_peer_id
            }),
            "an outbound local Nexus fanout leg must never re-enter local proxy delivery"
        );

        let exhausted = super::torii_proxy_candidate_peer_ids(
            app.as_ref(),
            &local_peer_id,
            route,
            None,
            &[
                authoritative_peer_id.clone(),
                sender_peer_id.clone(),
                visited_peer_id.clone(),
            ],
        );
        assert!(exhausted.peers.is_empty());
        assert_eq!(exhausted.authoritative_total_count, 3);
        assert_eq!(exhausted.loop_prevention_drops, 3);
        assert_eq!(
            exhausted.unavailable_reason,
            Some(ToriiProxyUnavailableReason::LoopPreventionExhausted)
        );
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn local_nexus_read_fanout_completes_without_recursive_self_proxying() {
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&ALICE_ID));
        let local_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x67,
                "derive local Nexus fanout peer fixture key",
            )
            .public_key()
            .clone(),
        );
        Arc::get_mut(&mut app)
            .expect("unique local Nexus fanout app")
            .local_peer_id = Some(local_peer_id.clone());
        let request = ToriiProxyRequestV5 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V5,
            request_id: Hash::new(b"local-nexus-fanout-terminates"),
            hop_count: 1,
            max_hops: TORII_PROXY_DEFAULT_MAX_HOPS,
            visited_peer_ids: vec![local_peer_id.clone()],
            request: ToriiProxyRequestKindV4::ReadFanout(super::torii_read_fanout_request(
                ToriiReadEndpointV1::AccountGet,
                ToriiFanoutRouteScopeV1::TargetAccount {
                    account_id: ALICE_ID.to_string(),
                },
                ToriiReadFanoutMergeV1::Account,
                vec![ALICE_ID.to_string()],
                None,
                Vec::new(),
                ToriiProxyResponseFormatV1::Json,
            )),
        };

        let snapshot = tokio::time::timeout(
            Duration::from_secs(2),
            super::execute_torii_proxy_request_locally(
                &app,
                local_peer_id,
                request,
            ),
        )
        .await
        .expect("local fanout must not recurse until timeout")
        .expect("local fanout snapshot");
        assert_eq!(snapshot.status_code, StatusCode::OK.as_u16());
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn incoming_torii_proxy_rejects_every_non_v5_schema_before_dispatch() {
        let app = mk_app_state_for_tests_with_world(world_with_account(&ALICE_ID));
        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        for schema_version in [0, 1, 2, 3, 4, u16::MAX] {
            let request = ToriiProxyRequestV5 {
                schema_version,
                request_id: Hash::new(
                    norito::to_bytes(&("reject-non-v5-torii-proxy-request", schema_version))
                        .expect("encode non-V5 request id"),
                ),
                hop_count: 1,
                max_hops: 3,
                visited_peer_ids: Vec::new(),
                request: ToriiProxyRequestKindV4::Read(super::torii_read_request(
                    ToriiReadEndpointV1::AccountGet,
                    route,
                    vec![ALICE_ID.to_string()],
                    None,
                    Vec::new(),
                )),
            };

            let response = super::execute_incoming_torii_proxy_request(&app, request, None).await;
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "schema version {schema_version} must fail before dispatch"
            );
        }
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn internal_torii_proxy_route_accepts_node_signed_requests() {
        use tower::ServiceExt as _;

        let mut app = mk_app_state_for_tests_with_world(world_with_account(&ALICE_ID));
        let bridge_signer = checked_torii_test_ed25519_keypair(
            0x67,
            "derive internal Torii HTTP bridge sender fixture key",
        );
        let receiver_signer = checked_torii_test_ed25519_keypair(
            0x68,
            "derive internal Torii HTTP bridge receiver fixture key",
        );
        let sender_peer_id = PeerId::from(bridge_signer.public_key().clone());
        let local_peer_id = PeerId::from(receiver_signer.public_key().clone());
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .torii_proxy_bridge_signer = bridge_signer.clone();
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .local_peer_id = Some(local_peer_id.clone());
        {
            let mut topology = app.state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.push(sender_peer_id.clone());
            topology.commit();
        }

        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let proxy_request = ToriiProxyRequestV5 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V5,
            request_id: Hash::new(b"internal-torii-proxy-read"),
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: vec![sender_peer_id],
            request: ToriiProxyRequestKindV4::Read(super::torii_read_request(
                ToriiReadEndpointV1::AccountGet,
                route,
                vec![ALICE_ID.to_string()],
                None,
                Vec::new(),
            )),
        };
        let body = norito::to_bytes(&proxy_request).expect("encode proxied read");
        let uri = TORII_INTERNAL_PROXY_HTTP_PATH
            .parse::<crate::Uri>()
            .expect("internal proxy URI");
        let signed_headers = operator_signatures::signed_torii_proxy_request_headers(
            &bridge_signer,
            app.state.network_id_ref(),
            &local_peer_id,
            &crate::Method::POST,
            &uri,
            &body,
        )
        .expect("target-bound Torii proxy signature headers");
        let peer_layer = axum::middleware::from_fn_with_state::<
            _,
            _,
            (axum::extract::State<SharedAppState>, axum::extract::Request),
        >(
            app.clone(),
            operator_signatures::enforce_torii_proxy_peer_signature,
        );
        let router = axum::Router::new()
            .route(
                TORII_INTERNAL_PROXY_HTTP_PATH,
                axum::routing::post(handler_internal_torii_proxy_request).layer(peer_layer),
            )
            .with_state(app.clone());
        let mut request = axum::http::Request::builder()
            .uri(TORII_INTERNAL_PROXY_HTTP_PATH)
            .method(axum::http::Method::POST)
            .body(axum::body::Body::from(body))
            .expect("request");
        request.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-norito"),
        );
        request.headers_mut().insert(
            axum::http::header::ACCEPT,
            HeaderValue::from_static("application/x-norito"),
        );
        request.headers_mut().extend(signed_headers);

        let response = router
            .clone()
            .oneshot(request)
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("proxy")
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        let payload = std::str::from_utf8(&body).expect("JSON response body");
        assert!(payload.contains(&ALICE_ID.to_string()));

        let untrusted_signer = checked_torii_test_ed25519_keypair(
            0x6b,
            "derive untrusted internal Torii HTTP bridge sender fixture key",
        );
        let untrusted_peer_id = PeerId::from(untrusted_signer.public_key().clone());
        let untrusted_body = norito::to_bytes(&ToriiProxyRequestV5 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V5,
            request_id: Hash::new(b"internal-torii-proxy-untrusted-peer"),
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: vec![untrusted_peer_id],
            request: ToriiProxyRequestKindV4::Read(super::torii_read_request(
                ToriiReadEndpointV1::AccountGet,
                route,
                vec![ALICE_ID.to_string()],
                None,
                Vec::new(),
            )),
        })
        .expect("encode untrusted proxied read");
        let untrusted_headers = operator_signatures::signed_torii_proxy_request_headers(
            &untrusted_signer,
            app.state.network_id_ref(),
            &local_peer_id,
            &crate::Method::POST,
            &uri,
            &untrusted_body,
        )
        .expect("sign target-bound request from untrusted peer");
        let mut untrusted_request = axum::http::Request::builder()
            .uri(TORII_INTERNAL_PROXY_HTTP_PATH)
            .method(axum::http::Method::POST)
            .body(axum::body::Body::from(untrusted_body))
            .expect("untrusted request");
        untrusted_request.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-norito"),
        );
        untrusted_request.headers_mut().extend(untrusted_headers);
        let untrusted_response = router
            .oneshot(untrusted_request)
            .await
            .expect("untrusted peer response");
        assert_eq!(untrusted_response.status(), StatusCode::FORBIDDEN);
    }

    #[cfg(all(feature = "app_api", any(feature = "p2p_ws", feature = "connect")))]
    #[tokio::test]
    async fn internal_torii_proxy_route_rejects_unsigned_requests() {
        use tower::ServiceExt as _;

        let mut app = mk_app_state_for_tests_with_world(world_with_account(&ALICE_ID));
        let local_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x69,
                "derive unsigned internal Torii proxy receiver fixture key",
            )
            .public_key()
            .clone(),
        );
        Arc::get_mut(&mut app)
            .expect("unsigned internal proxy fixture app must be uniquely owned")
            .local_peer_id = Some(local_peer_id);
        let peer_layer = axum::middleware::from_fn_with_state::<
            _,
            _,
            (axum::extract::State<SharedAppState>, axum::extract::Request),
        >(
            app.clone(),
            operator_signatures::enforce_torii_proxy_peer_signature,
        );
        let router = axum::Router::new()
            .route(
                TORII_INTERNAL_PROXY_HTTP_PATH,
                axum::routing::post(handler_internal_torii_proxy_request).layer(peer_layer),
            )
            .with_state(app.clone());
        let body = norito::to_bytes(&ToriiProxyRequestV5 {
            schema_version: TORII_PROXY_REQUEST_VERSION_V5,
            request_id: Hash::new(b"internal-torii-proxy-read-unsigned"),
            hop_count: 1,
            max_hops: 3,
            visited_peer_ids: vec![PeerId::from(
                checked_torii_test_ed25519_keypair(
                    0x6a,
                    "derive unsigned internal Torii proxy visited peer fixture key",
                )
                .public_key()
                .clone(),
            )],
            request: ToriiProxyRequestKindV4::Read(super::torii_read_request(
                ToriiReadEndpointV1::AccountGet,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                vec![ALICE_ID.to_string()],
                None,
                Vec::new(),
            )),
        })
        .expect("encode proxied read");

        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .uri(TORII_INTERNAL_PROXY_HTTP_PATH)
                    .method(axum::http::Method::POST)
                    .header(axum::http::header::CONTENT_TYPE, "application/x-norito")
                    .header(axum::http::header::ACCEPT, "application/x-norito")
                    .body(axum::body::Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn sorafs_por_proof_route_requires_fresh_path_bound_operator_signature() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};

        use tower::ServiceExt as _;

        let app = mk_app_state_for_tests();
        let signer = app.da_receipt_signer.clone();
        let operator_layer = axum::middleware::from_fn_with_state::<
            _,
            _,
            (axum::extract::State<SharedAppState>, axum::extract::Request),
        >(app.clone(), operator_signatures::enforce_operator_access);
        let router = axum::Router::new()
            .route(
                "/v1/sorafs/capacity/por-proof",
                axum::routing::post(handler_post_sorafs_capacity_por_proof)
                    .layer(operator_layer.clone()),
            )
            .route(
                "/v1/sorafs/capacity/por-verdict",
                axum::routing::post(handler_post_sorafs_capacity_por_verdict).layer(operator_layer),
            )
            .with_state(app);
        let body = br#"{"proof_b64":"not-base64%%"}"#.to_vec();
        let remote =
            axum::extract::ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 19_999));

        let unsigned = axum::http::Request::builder()
            .uri("/v1/sorafs/capacity/por-proof")
            .method(axum::http::Method::POST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .extension(remote)
            .body(axum::body::Body::from(body.clone()))
            .expect("unsigned PoR request");
        let unsigned_response = router
            .clone()
            .oneshot(unsigned)
            .await
            .expect("unsigned response");
        assert_eq!(unsigned_response.status(), StatusCode::UNAUTHORIZED);

        let uri = "/v1/sorafs/capacity/por-proof"
            .parse::<crate::Uri>()
            .expect("PoR proof URI");
        let signed_headers = operator_signatures::signed_request_headers(
            &signer,
            app.state.network_id_ref(),
            &crate::Method::POST,
            &uri,
            &body,
        )
        .expect("signed PoR headers");
        let signed_request = || {
            let mut request = axum::http::Request::builder()
                .uri(uri.clone())
                .method(axum::http::Method::POST)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .extension(remote)
                .body(axum::body::Body::from(body.clone()))
                .expect("signed PoR request");
            request.headers_mut().extend(signed_headers.clone());
            request
        };

        let accepted_auth = router
            .clone()
            .oneshot(signed_request())
            .await
            .expect("authenticated response");
        assert_eq!(accepted_auth.status(), StatusCode::BAD_REQUEST);

        let replay = router
            .clone()
            .oneshot(signed_request())
            .await
            .expect("replay response");
        assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);

        let mut cross_path = axum::http::Request::builder()
            .uri("/v1/sorafs/capacity/por-verdict")
            .method(axum::http::Method::POST)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .extension(remote)
            .body(axum::body::Body::from(body))
            .expect("cross-path PoR request");
        cross_path.headers_mut().extend(signed_headers);
        let cross_path_response = router
            .oneshot(cross_path)
            .await
            .expect("cross-path response");
        assert_eq!(cross_path_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn forward_incoming_torii_proxy_request_returns_route_unavailable_when_hops_exhausted() {
        let local_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x68,
                "derive internal Torii proxy local peer fixture key",
            )
            .public_key()
            .clone(),
        );
        let sender_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x6a,
                "derive internal Torii proxy sender peer fixture key",
            )
            .public_key()
            .clone(),
        );
        let mut app = mk_app_state_for_tests();
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .local_peer_id = Some(local_peer_id.clone());

        let route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(1));
        let response = super::forward_incoming_torii_proxy_request(
            &app,
            &sender_peer_id,
            route,
            &ToriiProxyRequestV5 {
                schema_version: TORII_PROXY_REQUEST_VERSION_V5,
                request_id: Hash::new(b"torii-proxy-hop-exhausted"),
                hop_count: 1,
                max_hops: 1,
                visited_peer_ids: vec![sender_peer_id.clone()],
                request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
                    query_bytes: Vec::new(),
                    expected_route: ToriiRouteHintV1::from(route),
                    response_format: ToriiProxyResponseFormatV1::Norito,
                },
            },
        )
        .await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn forward_incoming_torii_proxy_request_reaches_authoritative_peer() {
        let local_keypair = checked_torii_test_keypair_from_seed_byte(
            0x6b,
            Algorithm::BlsNormal,
            "derive internal Torii forward local peer fixture key",
        );
        let authoritative_keypair = checked_torii_test_keypair_from_seed_byte(
            0x6c,
            Algorithm::BlsNormal,
            "derive internal Torii forward authoritative peer fixture key",
        );
        let sender_keypair = checked_torii_test_keypair_from_seed_byte(
            0x6d,
            Algorithm::BlsNormal,
            "derive internal Torii forward sender peer fixture key",
        );
        let local_peer_id = PeerId::from(local_keypair.public_key().clone());
        let authoritative_peer_id = PeerId::from(authoritative_keypair.public_key().clone());
        let sender_peer_id = PeerId::from(sender_keypair.public_key().clone());
        let authoritative_validator = checked_torii_test_account_id(
            0x6e,
            "derive internal Torii forward authoritative validator fixture key",
        );
        let sender_validator = checked_torii_test_account_id(
            0x6f,
            "derive internal Torii forward sender validator fixture key",
        );
        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let (online_tx, online_rx) =
                tokio::sync::watch::channel(std::collections::HashSet::new());
            online_tx
                .send(std::collections::HashSet::from([
                    Peer::new(
                        "127.0.0.1:10001".parse().expect("valid local address"),
                        local_keypair.public_key().clone(),
                    ),
                    Peer::new(
                        "127.0.0.1:10002"
                            .parse()
                            .expect("valid authoritative address"),
                        authoritative_keypair.public_key().clone(),
                    ),
                ]))
                .expect("online peers update should succeed");
            app_mut.online_peers = OnlinePeersProvider::new(online_rx);
            app_mut.local_peer_id = Some(local_peer_id.clone());
            app_mut.p2p = Some(iroha_core::IrohaNetwork::closed_for_tests());
        }

        {
            let mut topology = app.state.commit_topology.block();
            topology.clear();
            topology.push(authoritative_peer_id.clone());
            topology.commit();
        }
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
            ensure_runtime_peer_binding_for_test(
                state,
                &authoritative_validator,
                &authoritative_keypair,
                "authoritative",
            );
            ensure_runtime_peer_binding_for_test(
                state,
                &sender_validator,
                &sender_keypair,
                "sender",
            );
            install_lane_manifest_registry_for_test(
                state,
                &[(
                    LaneId::SINGLE,
                    vec![
                        (authoritative_validator, authoritative_peer_id.clone()),
                        (sender_validator, sender_peer_id.clone()),
                    ],
                )],
            );
        }

        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        let request_id = Hash::new(b"torii-proxy-forward-success");
        let app_for_response = app.clone();
        let authoritative_peer_for_response = authoritative_peer_id.clone();
        let request_id_for_response = request_id.clone();
        let response_task = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    let pending = app_for_response.torii_proxy_pending.lock().await;
                    if pending.contains_key(&(
                        request_id_for_response,
                        authoritative_peer_for_response.clone(),
                    )) {
                        break;
                    }
                    drop(pending);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("forwarded Torii proxy request should become pending");

            super::process_incoming_torii_proxy_response(
                &app_for_response,
                authoritative_peer_for_response,
                ToriiProxyResponseV1 {
                    schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
                    request_id: request_id_for_response,
                    response: ToriiProxyHttpResponseV1 {
                        status_code: StatusCode::OK.as_u16(),
                        headers: Vec::new(),
                        body: b"forwarded-ok".to_vec(),
                    },
                },
            )
            .await;
        });

        let response = super::forward_incoming_torii_proxy_request(
            &app,
            &sender_peer_id,
            route,
            &ToriiProxyRequestV5 {
                schema_version: TORII_PROXY_REQUEST_VERSION_V5,
                request_id,
                hop_count: 1,
                max_hops: 3,
                visited_peer_ids: vec![sender_peer_id.clone()],
                request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
                    query_bytes: Vec::new(),
                    expected_route: ToriiRouteHintV1::from(route),
                    response_format: ToriiProxyResponseFormatV1::Norito,
                },
            },
        )
        .await;
        response_task
            .await
            .expect("proxy response task should complete");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("forwarded response body should be readable");
        assert_eq!(body.as_ref(), b"forwarded-ok");
    }

    #[tokio::test]
    async fn validate_incoming_soracloud_proxy_request_authority_accepts_generated_hf_primary() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(primary_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "authority validation test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }));

        let result = super::validate_incoming_soracloud_proxy_request_authority(
            &app,
            &sample_generated_hf_infer_request(service_name, service_version),
        );

        assert!(
            result.is_ok(),
            "generated HF primary proxy request must validate"
        );
    }

    #[tokio::test]
    async fn validate_incoming_soracloud_proxy_request_authority_rejects_commitment_mismatch() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(primary_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "commitment validation test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }));

        let mut request = sample_generated_hf_infer_request(service_name, service_version);
        request.request_commitment = Hash::new(b"forged-generated-hf-proxy-request");
        let error = super::validate_incoming_soracloud_proxy_request_authority(&app, &request)
            .expect_err("mismatched proxy request commitment must fail closed");

        assert_eq!(
            error.kind,
            SoracloudRuntimeExecutionErrorKind::InvalidRequest
        );
        assert!(error.message.contains("request commitment"));
    }

    #[tokio::test]
    async fn validate_incoming_soracloud_proxy_request_authority_rejects_non_generated_hf() {
        let mut app = mk_app_state_for_tests_with_world(seed_public_soracloud_world());
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(
                PeerId::from(
                    checked_torii_test_ed25519_keypair(
                        0x72,
                        "derive non-generated incoming proxy local fixture key",
                    )
                    .public_key()
                    .clone(),
                )
                .to_string(),
            ),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "authority validation test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }));

        let error = super::validate_incoming_soracloud_proxy_request_authority(
            &app,
            &sample_public_query_request("web_portal".to_owned(), "2026.02.0".to_owned()),
        )
        .expect_err("non-generated public routes must be rejected over proxy");

        assert_eq!(
            error.kind,
            SoracloudRuntimeExecutionErrorKind::InvalidRequest
        );
        assert!(
            error
                .message
                .contains("generated HF `infer` query handlers")
        );
    }

    #[tokio::test]
    async fn validate_incoming_soracloud_proxy_request_authority_rejects_non_primary_peer() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let local_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x71,
                "derive generated-HF incoming proxy local fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "authority validation test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }));

        let error = super::validate_incoming_soracloud_proxy_request_authority(
            &app,
            &sample_generated_hf_infer_request(service_name, service_version),
        )
        .expect_err("non-primary peer must reject proxy execution");

        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        assert!(error.message.contains("not the authoritative warm primary"));
    }

    #[tokio::test]
    async fn incoming_proxy_authority_failure_requests_generated_hf_reconcile() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let local_peer_id =
            active_generated_hf_replica_peer_id(&world, &service_name, &service_version);
        let captured_reconcile_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "incoming authority failure test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::clone(&captured_reconcile_requests),
        }));

        let request = sample_generated_hf_infer_request(service_name, service_version);
        let error = super::validate_incoming_soracloud_proxy_request_authority(&app, &request)
            .expect_err("non-primary peer must reject proxy execution");
        super::handle_incoming_soracloud_generated_hf_proxy_authority_failure(
            &app, &request, &error,
        );

        let captured = captured_reconcile_requests
            .lock()
            .expect("reconcile capture lock");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].0.service_name, request.service_name);
        assert_eq!(captured[0].0.service_version, request.service_version);
        assert_eq!(
            captured[0].1.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(
            captured[0]
                .1
                .message
                .contains("not the authoritative warm primary")
        );
    }

    #[tokio::test]
    async fn incoming_proxy_forward_failure_reports_remote_primary_health_and_requests_reconcile_once()
     {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let local_peer_id =
            active_generated_hf_replica_peer_id(&world, &service_name, &service_version);
        let captured_proxy_failures = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_reconcile_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "incoming forward failure test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::clone(&captured_proxy_failures),
            captured_reconcile_requests: Arc::clone(&captured_reconcile_requests),
        }));
        Arc::get_mut(&mut app).expect("unique app state").p2p =
            Some(iroha_core::IrohaNetwork::closed_for_tests());

        let request = sample_generated_hf_infer_request(service_name, service_version);
        let incoming_request = SoracloudLocalReadProxyRequestV1 {
            schema_version: SORACLOUD_LOCAL_READ_PROXY_REQUEST_VERSION_V1,
            request_id: Hash::new(b"incoming-generated-hf-forward-failure"),
            request: request.clone(),
        };
        let response_error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "replica forwarding to primary timed out",
        );
        let app_for_response = app.clone();
        let primary_peer_id_for_response: PeerId =
            primary_peer_id.parse().expect("valid primary peer id");
        let response_task = tokio::spawn(async move {
            let request_id = tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if let Some(request_id) = app_for_response
                        .soracloud_proxy_pending
                        .lock()
                        .await
                        .keys()
                        .next()
                        .copied()
                    {
                        break request_id;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("forwarded proxy request should be pending before timeout");
            super::process_incoming_soracloud_proxy_response(
                &app_for_response,
                primary_peer_id_for_response,
                SoracloudLocalReadProxyResponseV1 {
                    schema_version: SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1,
                    request_id,
                    outcome: SoracloudLocalReadProxyOutcomeV1::Err(response_error),
                },
            )
            .await;
        });

        super::process_incoming_soracloud_proxy_request(
            app.clone(),
            iroha_core::IrohaNetwork::closed_for_tests(),
            Peer::new(
                "127.0.0.1:1337".parse().expect("valid peer address"),
                checked_torii_test_ed25519_keypair(
                    0x73,
                    "derive generated-HF incoming proxy sender fixture key",
                )
                .public_key()
                .clone(),
            ),
            incoming_request,
        )
        .await;
        response_task
            .await
            .expect("proxy response task should succeed");

        let proxy_failures = captured_proxy_failures
            .lock()
            .expect("proxy failure capture lock");
        assert_eq!(proxy_failures.len(), 1);
        assert_eq!(proxy_failures[0].0.service_name, request.service_name);
        assert_eq!(proxy_failures[0].1, primary_peer_id);
        assert_eq!(
            proxy_failures[0].2.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(proxy_failures[0].2.message.contains("timed out"));

        let captured = captured_reconcile_requests
            .lock()
            .expect("reconcile capture lock");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].0.service_name, request.service_name);
        assert_eq!(captured[0].0.service_version, request.service_version);
        assert_eq!(
            captured[0].1.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(captured[0].1.message.contains("timed out"));
    }

    #[tokio::test]
    async fn resolve_incoming_soracloud_proxy_forward_target_returns_primary() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let local_peer_id =
            active_generated_hf_replica_peer_id(&world, &service_name, &service_version);
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "forward target resolution test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }));
        let request = sample_generated_hf_infer_request(service_name, service_version);
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "local peer is no longer the authoritative warm primary",
        );

        let forward_target =
            super::resolve_incoming_soracloud_proxy_forward_target(&app, &request, &error)
                .expect("non-primary generated-HF receiver should forward to primary");

        assert_eq!(forward_target.to_string(), primary_peer_id);
    }

    #[tokio::test]
    async fn resolve_incoming_soracloud_proxy_forward_target_rejects_unassigned_receiver() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let local_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x71,
                "derive generated-HF incoming proxy local fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "unassigned receiver test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }));
        let request = sample_generated_hf_infer_request(service_name, service_version);
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "local peer is no longer the authoritative warm primary",
        );

        let forward_target =
            super::resolve_incoming_soracloud_proxy_forward_target(&app, &request, &error);

        assert!(
            forward_target.is_none(),
            "unassigned receivers must not act as generated-HF proxy intermediaries"
        );
    }

    #[tokio::test]
    async fn resolve_incoming_soracloud_proxy_forward_target_rejects_invalid_request() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(primary_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "forward target resolution test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }));
        let request = sample_generated_hf_infer_request(service_name, service_version);
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::InvalidRequest,
            "proxy request commitment is invalid",
        );

        let forward_target =
            super::resolve_incoming_soracloud_proxy_forward_target(&app, &request, &error);

        assert!(forward_target.is_none());
    }

    #[tokio::test]
    async fn soracloud_proxy_response_ignores_unexpected_responder_and_keeps_pending_request() {
        let app = mk_app_state_for_tests();
        let request_id = Hash::new(b"soracloud-proxy-unexpected-peer");
        let expected_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x74,
                "derive Soracloud proxy expected responder fixture key",
            )
            .public_key()
            .clone(),
        );
        let responder_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x75,
                "derive Soracloud proxy unexpected responder fixture key",
            )
            .public_key()
            .clone(),
        );
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut rx = Box::pin(rx);
        app.soracloud_proxy_pending.lock().await.insert(
            request_id,
            PendingSoracloudProxyRequest {
                sender: tx,
                expected_peer_id: expected_peer_id.clone(),
                request: sample_public_query_request(
                    "web_portal".to_owned(),
                    "2026.02.0".to_owned(),
                ),
            },
        );

        super::process_incoming_soracloud_proxy_response(
            &app,
            responder_peer_id.clone(),
            SoracloudLocalReadProxyResponseV1 {
                schema_version: SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1,
                request_id,
                outcome: SoracloudLocalReadProxyOutcomeV1::Ok(
                    iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
                        response_bytes: b"unexpected".to_vec(),
                        content_type: None,
                        content_encoding: None,
                        cache_control: None,
                        bindings: Vec::new(),
                        result_commitment: Hash::new(b"unexpected"),
                        certified_by:
                            iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                        runtime_receipt: None,
                    },
                ),
            },
        )
        .await;

        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut rx)
                .await
                .is_err(),
            "unexpected responder must not complete the pending proxy request"
        );
        assert!(
            app.soracloud_proxy_pending
                .lock()
                .await
                .contains_key(&request_id),
            "unexpected responder must not poison the pending proxy request"
        );

        let replacement_response = iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
            response_bytes: b"authoritative".to_vec(),
            content_type: None,
            content_encoding: None,
            cache_control: None,
            bindings: Vec::new(),
            result_commitment: Hash::new(b"authoritative"),
            certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
            runtime_receipt: None,
        };
        super::process_incoming_soracloud_proxy_response(
            &app,
            expected_peer_id.clone(),
            SoracloudLocalReadProxyResponseV1 {
                schema_version: SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1,
                request_id,
                outcome: SoracloudLocalReadProxyOutcomeV1::Ok(replacement_response.clone()),
            },
        )
        .await;

        let pending_result = app
            .soracloud_proxy_pending
            .lock()
            .await
            .contains_key(&request_id);
        assert!(
            !pending_result,
            "expected responder must resolve and clear the pending proxy request"
        );
        match rx.await.expect("pending proxy request should resolve") {
            SoracloudLocalReadProxyOutcomeV1::Ok(delivered) => {
                assert_eq!(
                    delivered.response_bytes,
                    replacement_response.response_bytes
                );
                assert_eq!(
                    delivered.result_commitment,
                    replacement_response.result_commitment
                );
            }
            SoracloudLocalReadProxyOutcomeV1::Err(error) => {
                panic!("expected authoritative response, got error: {error:?}")
            }
        }
    }

    #[tokio::test]
    async fn unexpected_assigned_proxy_responder_requests_generated_hf_reconcile() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x70,
                "derive generated-HF incoming proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let unexpected_peer_id =
            active_generated_hf_replica_peer_id(&world, &service_name, &service_version);
        let captured_reconcile_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(
                PeerId::from(
                    checked_torii_test_ed25519_keypair(
                        0x76,
                        "derive generated-HF unexpected responder local fixture key",
                    )
                    .public_key()
                    .clone(),
                )
                .to_string(),
            ),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "unexpected responder reconcile test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::clone(&captured_reconcile_requests),
        }));

        let request = sample_generated_hf_infer_request(service_name, service_version);
        let request_id = Hash::new(b"generated-hf-unexpected-assigned-responder");
        let (tx, _rx) = tokio::sync::oneshot::channel();
        app.soracloud_proxy_pending.lock().await.insert(
            request_id,
            PendingSoracloudProxyRequest {
                sender: tx,
                expected_peer_id: primary_peer_id.parse().expect("valid primary peer id"),
                request: request.clone(),
            },
        );

        super::process_incoming_soracloud_proxy_response(
            &app,
            unexpected_peer_id
                .parse()
                .expect("valid assigned replica peer id"),
            SoracloudLocalReadProxyResponseV1 {
                schema_version: SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1,
                request_id,
                outcome: SoracloudLocalReadProxyOutcomeV1::Ok(
                    iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
                        response_bytes: b"unexpected-assigned-responder".to_vec(),
                        content_type: None,
                        content_encoding: None,
                        cache_control: None,
                        bindings: Vec::new(),
                        result_commitment: Hash::new(b"unexpected-assigned-responder"),
                        certified_by:
                            iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                        runtime_receipt: None,
                    },
                ),
            },
        )
        .await;

        assert!(
            app.soracloud_proxy_pending
                .lock()
                .await
                .contains_key(&request_id),
            "unexpected assigned responder must not poison the pending proxy request"
        );

        let reconcile_requests = captured_reconcile_requests
            .lock()
            .expect("reconcile capture lock");
        assert_eq!(reconcile_requests.len(), 1);
        assert_eq!(reconcile_requests[0].0.service_name, request.service_name);
        assert_eq!(
            reconcile_requests[0].1.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(
            reconcile_requests[0]
                .1
                .message
                .contains("unexpected proxy responder")
        );
    }

    #[tokio::test]
    async fn soracloud_proxy_response_rejects_unsupported_schema() {
        let app = mk_app_state_for_tests();
        let request_id = Hash::new(b"soracloud-proxy-unsupported-schema");
        let responder_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x77,
                "derive Soracloud proxy unsupported schema responder fixture key",
            )
            .public_key()
            .clone(),
        );
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.soracloud_proxy_pending.lock().await.insert(
            request_id,
            PendingSoracloudProxyRequest {
                sender: tx,
                expected_peer_id: responder_peer_id.clone(),
                request: sample_public_query_request(
                    "web_portal".to_owned(),
                    "2026.02.0".to_owned(),
                ),
            },
        );

        super::process_incoming_soracloud_proxy_response(
            &app,
            responder_peer_id,
            SoracloudLocalReadProxyResponseV1 {
                schema_version: SORACLOUD_LOCAL_READ_PROXY_RESPONSE_VERSION_V1 + 1,
                request_id,
                outcome: SoracloudLocalReadProxyOutcomeV1::Ok(
                    iroha_core::soracloud_runtime::SoracloudLocalReadResponse {
                        response_bytes: b"unexpected".to_vec(),
                        content_type: None,
                        content_encoding: None,
                        cache_control: None,
                        bindings: Vec::new(),
                        result_commitment: Hash::new(b"unexpected"),
                        certified_by:
                            iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                        runtime_receipt: None,
                    },
                ),
            },
        )
        .await;

        match rx.await.expect("pending proxy request should resolve") {
            SoracloudLocalReadProxyOutcomeV1::Err(error) => {
                assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
                assert!(error.message.contains("unsupported schema_version"));
            }
            SoracloudLocalReadProxyOutcomeV1::Ok(_) => {
                panic!("unsupported proxy schema must fail closed")
            }
        }
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn torii_proxy_network_message_dispatch_resolves_pending_response() {
        let app = mk_app_state_for_tests();
        let request_id = Hash::new(b"torii-proxy-dispatch");
        let responder_keypair = checked_torii_test_ed25519_keypair(
            0x78,
            "derive Torii proxy response dispatcher responder fixture key",
        );
        let responder_peer_id = PeerId::from(responder_keypair.public_key().clone());
        let responder_peer = Peer::new(
            "127.0.0.1:1337".parse().expect("valid peer address"),
            responder_keypair.public_key().clone(),
        );
        let expected_response = ToriiProxyHttpResponseV1 {
            status_code: StatusCode::ACCEPTED.as_u16(),
            headers: Vec::new(),
            body: b"torii-proxy-response".to_vec(),
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.torii_proxy_pending.lock().await.insert(
            (request_id, responder_peer_id),
            PendingToriiProxyRequest {
                sender: tx,
                max_body_bytes: usize::MAX,
                strict_queue_plan_synced: false,
            },
        );

        super::handle_torii_proxy_network_message(
            app.clone(),
            iroha_core::IrohaNetwork::closed_for_tests(),
            responder_peer,
            iroha_core::NetworkMessage::ToriiProxyResponse(Box::new(ToriiProxyResponseV1 {
                schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
                request_id,
                response: expected_response.clone(),
            })),
        )
        .await;

        assert_eq!(
            rx.await
                .expect("pending Torii proxy request should resolve"),
            expected_response
        );
        assert!(
            !app.torii_proxy_pending
                .lock()
                .await
                .keys()
                .any(|(pending_request_id, _)| *pending_request_id == request_id),
            "Torii proxy response dispatcher must clear the pending request"
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[tokio::test]
    async fn process_incoming_torii_proxy_response_marks_late_responses_once() {
        let app = mk_app_state_for_tests();
        let request_id = Hash::new(b"torii-proxy-late-response");
        let responder_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x79,
                "derive Torii proxy late-response responder fixture key",
            )
            .public_key()
            .clone(),
        );
        super::mark_torii_proxy_request_completed(&app, request_id).await;

        let response = ToriiProxyResponseV1 {
            schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
            request_id,
            response: ToriiProxyHttpResponseV1 {
                status_code: StatusCode::OK.as_u16(),
                headers: Vec::new(),
                body: b"late".to_vec(),
            },
        };
        super::process_incoming_torii_proxy_response(&app, responder_peer_id.clone(), response)
            .await;
        {
            let completed = app.torii_proxy_completed.lock().await;
            assert!(
                completed
                    .entries
                    .get(&request_id)
                    .is_some_and(|entry| entry.late_response_logged),
                "first late response should latch the one-time ignore marker"
            );
        }

        super::process_incoming_torii_proxy_response(
            &app,
            responder_peer_id,
            ToriiProxyResponseV1 {
                schema_version: TORII_PROXY_RESPONSE_VERSION_V1,
                request_id,
                response: ToriiProxyHttpResponseV1 {
                    status_code: StatusCode::OK.as_u16(),
                    headers: Vec::new(),
                    body: b"late-again".to_vec(),
                },
            },
        )
        .await;
        assert!(
            app.torii_proxy_pending.lock().await.is_empty(),
            "late responses must not recreate pending state after completion"
        );
    }

    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    #[test]
    fn completed_torii_proxy_requests_are_fifo_bounded_and_ttl_pruned() {
        let request_id = |index: usize| {
            let mut bytes = [0_u8; Hash::LENGTH];
            bytes[..8].copy_from_slice(
                &u64::try_from(index)
                    .expect("bounded completion-cache test index")
                    .to_le_bytes(),
            );
            Hash::prehashed(bytes)
        };
        let now = Instant::now();
        let mut completed = CompletedToriiProxyRequests::default();
        for index in 0..=TORII_PROXY_COMPLETED_CAP {
            completed.record(request_id(index), now);
        }
        assert_eq!(completed.entries.len(), TORII_PROXY_COMPLETED_CAP);
        assert!(
            !completed.entries.contains_key(&request_id(0)),
            "capacity overflow must evict the oldest completed request"
        );
        assert!(
            completed
                .entries
                .contains_key(&request_id(TORII_PROXY_COMPLETED_CAP)),
            "capacity overflow must retain the newest completed request"
        );

        completed.prune(now + TORII_PROXY_COMPLETED_TTL);
        assert!(completed.entries.is_empty());
        assert!(completed.insertion_order.is_empty());
    }

    #[tokio::test]
    async fn soracloud_proxy_failure_reports_remote_primary_health() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x7a,
                "derive generated-HF proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let local_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(0x7b, "derive generated-HF proxy local fixture key")
                .public_key()
                .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let captured_proxy_failures = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "proxy test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::clone(&captured_proxy_failures),
            captured_reconcile_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        }));

        let request = sample_generated_hf_infer_request(service_name, service_version);
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "proxy request timed out",
        );
        super::report_soracloud_local_read_proxy_failure(
            &app,
            &request,
            &primary_peer_id.parse().expect("valid peer id"),
            &error,
        );

        let reports = captured_proxy_failures
            .lock()
            .expect("proxy failure capture lock");
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].0.service_name, request.service_name);
        assert_eq!(reports[0].1, primary_peer_id);
        assert_eq!(
            reports[0].2.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(reports[0].2.message.contains("timed out"));
    }

    #[tokio::test]
    async fn validate_generated_hf_proxy_response_authority_accepts_matching_receipt() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x7a,
                "derive generated-HF proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let request = sample_generated_hf_infer_request(service_name, service_version);
        let app = mk_app_state_for_tests_with_world(world);
        let response = sample_generated_hf_proxy_response(&app, &request);

        let result = super::validate_generated_hf_proxy_response_authority(
            &app,
            &request,
            &primary_peer_id.parse().expect("valid peer id"),
            &response,
        );

        assert!(result.is_ok(), "matching proxy receipt must validate");
    }

    #[tokio::test]
    async fn validate_generated_hf_proxy_response_authority_rejects_mismatched_receipt() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x7a,
                "derive generated-HF proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let request = sample_generated_hf_infer_request(service_name, service_version);
        let app = mk_app_state_for_tests_with_world(world);
        let mut response = sample_generated_hf_proxy_response(&app, &request);
        response
            .runtime_receipt
            .as_mut()
            .expect("generated HF proxy receipt")
            .selected_peer_id = Some(
            PeerId::from(
                checked_torii_test_ed25519_keypair(
                    0x7c,
                    "derive generated-HF proxy mismatched receipt fixture key",
                )
                .public_key()
                .clone(),
            )
            .to_string(),
        );

        let error = super::validate_generated_hf_proxy_response_authority(
            &app,
            &request,
            &primary_peer_id.parse().expect("valid peer id"),
            &response,
        )
        .expect_err("mismatched proxy receipt must fail closed");

        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        assert!(error.message.contains("receipt attribution"));
    }

    #[tokio::test]
    async fn validate_generated_hf_proxy_response_authority_rejects_result_commitment_mismatch() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x7a,
                "derive generated-HF proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let request = sample_generated_hf_infer_request(service_name, service_version);
        let app = mk_app_state_for_tests_with_world(world);
        let mut response = sample_generated_hf_proxy_response(&app, &request);
        response
            .runtime_receipt
            .as_mut()
            .expect("generated HF proxy receipt")
            .result_commitment = Hash::new(b"mismatched-generated-hf-proxy-result");

        let error = super::validate_generated_hf_proxy_response_authority(
            &app,
            &request,
            &primary_peer_id.parse().expect("valid peer id"),
            &response,
        )
        .expect_err("receipt and response commitments must agree");

        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        assert!(error.message.contains("response commitments"));
    }

    #[tokio::test]
    async fn proxy_validation_failure_reports_remote_primary_health_and_requests_reconcile() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x7a,
                "derive generated-HF proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let local_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(0x7b, "derive generated-HF proxy local fixture key")
                .public_key()
                .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let captured_proxy_failures = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_reconcile_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "validation failure test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::clone(&captured_proxy_failures),
            captured_reconcile_requests: Arc::clone(&captured_reconcile_requests),
        }));

        let request = sample_generated_hf_infer_request(service_name, service_version);
        let mut response = sample_generated_hf_proxy_response(&app, &request);
        response
            .runtime_receipt
            .as_mut()
            .expect("generated HF proxy receipt")
            .selected_peer_id = Some(
            PeerId::from(
                checked_torii_test_ed25519_keypair(
                    0x7c,
                    "derive generated-HF proxy mismatched receipt fixture key",
                )
                .public_key()
                .clone(),
            )
            .to_string(),
        );
        let error = super::validate_generated_hf_proxy_response_authority(
            &app,
            &request,
            &primary_peer_id.parse().expect("valid peer id"),
            &response,
        )
        .expect_err("mismatched proxy receipt must fail closed");

        super::handle_soracloud_generated_hf_proxy_validation_failure(
            &app,
            &request,
            &primary_peer_id.parse().expect("valid peer id"),
            &error,
        );

        let proxy_failures = captured_proxy_failures
            .lock()
            .expect("proxy failure capture lock");
        assert_eq!(proxy_failures.len(), 1);
        assert_eq!(proxy_failures[0].0.service_name, request.service_name);
        assert_eq!(proxy_failures[0].1, primary_peer_id);
        assert_eq!(
            proxy_failures[0].2.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(proxy_failures[0].2.message.contains("receipt attribution"));

        let reconcile_requests = captured_reconcile_requests
            .lock()
            .expect("reconcile capture lock");
        assert_eq!(reconcile_requests.len(), 1);
        assert_eq!(reconcile_requests[0].0.service_name, request.service_name);
        assert_eq!(
            reconcile_requests[0].1.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(
            reconcile_requests[0]
                .1
                .message
                .contains("receipt attribution")
        );
    }

    #[tokio::test]
    async fn proxy_execution_failure_reports_remote_primary_health_and_requests_reconcile() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x7a,
                "derive generated-HF proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let local_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(0x7b, "derive generated-HF proxy local fixture key")
                .public_key()
                .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let captured_proxy_failures = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_reconcile_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "execution failure test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::clone(&captured_proxy_failures),
            captured_reconcile_requests: Arc::clone(&captured_reconcile_requests),
        }));

        let request = sample_generated_hf_infer_request(service_name, service_version);
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "primary proxy request timed out",
        );
        super::handle_soracloud_generated_hf_proxy_execution_failure(
            &app,
            &request,
            &primary_peer_id.parse().expect("valid peer id"),
            &error,
        );

        let proxy_failures = captured_proxy_failures
            .lock()
            .expect("proxy failure capture lock");
        assert_eq!(proxy_failures.len(), 1);
        assert_eq!(proxy_failures[0].0.service_name, request.service_name);
        assert_eq!(proxy_failures[0].1, primary_peer_id);
        assert_eq!(
            proxy_failures[0].2.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(proxy_failures[0].2.message.contains("timed out"));

        let reconcile_requests = captured_reconcile_requests
            .lock()
            .expect("reconcile capture lock");
        assert_eq!(reconcile_requests.len(), 1);
        assert_eq!(reconcile_requests[0].0.service_name, request.service_name);
        assert_eq!(
            reconcile_requests[0].1.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(reconcile_requests[0].1.message.contains("timed out"));
    }

    #[tokio::test]
    async fn execute_soracloud_local_read_via_proxy_missing_p2p_does_not_blame_primary() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x7a,
                "derive generated-HF proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let local_peer_id =
            active_generated_hf_replica_peer_id(&world, &service_name, &service_version);
        let captured_proxy_failures = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_reconcile_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: Some(local_peer_id),
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "missing-p2p proxy test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::clone(&captured_proxy_failures),
            captured_reconcile_requests: Arc::clone(&captured_reconcile_requests),
        }));

        let request = sample_generated_hf_infer_request(service_name, service_version);
        let error = super::execute_soracloud_local_read_via_proxy(
            &app,
            primary_peer_id.parse().expect("valid peer id"),
            request.clone(),
        )
        .await
        .expect_err("missing P2P network must fail closed");

        assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
        assert!(error.message.contains("attached P2P network"));

        let proxy_failures = captured_proxy_failures
            .lock()
            .expect("proxy failure capture lock");
        assert!(
            proxy_failures.is_empty(),
            "local proxy transport failure must not blame the authoritative primary"
        );

        let reconcile_requests = captured_reconcile_requests
            .lock()
            .expect("reconcile capture lock");
        assert_eq!(reconcile_requests.len(), 1);
        assert_eq!(reconcile_requests[0].0.service_name, request.service_name);
        assert_eq!(
            reconcile_requests[0].1.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(
            reconcile_requests[0]
                .1
                .message
                .contains("attached P2P network")
        );
    }

    #[tokio::test]
    async fn soracloud_routing_failure_requests_generated_hf_reconcile() {
        let primary_peer_id = PeerId::from(
            checked_torii_test_ed25519_keypair(
                0x7a,
                "derive generated-HF proxy primary fixture key",
            )
            .public_key()
            .clone(),
        )
        .to_string();
        let (world, service_name, service_version) =
            seed_generated_hf_public_world(&primary_peer_id);
        let captured_reconcile_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = mk_app_state_for_tests_with_world(world);
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .soracloud_runtime = Some(Arc::new(TestLocalReadRuntime {
            snapshot: iroha_core::soracloud_runtime::SoracloudRuntimeSnapshot::default(),
            state_dir: PathBuf::from("/tmp/test-soracloud-runtime"),
            local_peer_id: None,
            result: Err(
                iroha_core::soracloud_runtime::SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "routing failure test should not execute the runtime locally",
                ),
            ),
            captured_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_proxy_failures: Arc::new(std::sync::Mutex::new(Vec::new())),
            captured_reconcile_requests: Arc::clone(&captured_reconcile_requests),
        }));

        let request = sample_generated_hf_infer_request(service_name, service_version);
        let error = SoracloudRuntimeExecutionError::new(
            SoracloudRuntimeExecutionErrorKind::Unavailable,
            "generated HF service has no warm authoritative primary host",
        );
        super::maybe_request_soracloud_generated_hf_reconcile(&app, &request, &error);

        let captured = captured_reconcile_requests
            .lock()
            .expect("reconcile capture lock");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].0.service_name, request.service_name);
        assert_eq!(captured[0].0.service_version, request.service_version);
        assert_eq!(
            captured[0].1.kind,
            SoracloudRuntimeExecutionErrorKind::Unavailable
        );
        assert!(captured[0].1.message.contains("warm authoritative primary"));
    }

    fn sccp_ingress_test_router_with_limit(
        app: SharedAppState,
        operator_max_body_bytes: usize,
    ) -> axum::Router {
        use axum::{Router, routing::post};

        seed_sccp_ingress_auth_account(&app);

        Router::new()
            .route(
                "/v1/bridge/proofs/submit",
                post(
                    |axum::Extension(admission): axum::Extension<
                        Arc<super::SccpSubmitAdmission>,
                    >,
                     _: axum::body::Bytes| async move {
                        admission.take().expect("take SCCP test admission");
                        StatusCode::NO_CONTENT
                    },
                ),
            )
            .route(
                "/v1/bridge/messages",
                post(
                    |axum::Extension(admission): axum::Extension<
                        Arc<super::SccpSubmitAdmission>,
                    >,
                     _: axum::body::Bytes| async move {
                        admission.take().expect("take SCCP test admission");
                        StatusCode::NO_CONTENT
                    },
                ),
            )
            .layer(axum::extract::DefaultBodyLimit::disable())
            .layer(axum::middleware::from_fn_with_state(
                super::SccpSubmitIngressState {
                    app,
                    operator_max_body_bytes,
                },
                super::enforce_sccp_submit_ingress,
            ))
    }

    fn sccp_ingress_test_router(app: SharedAppState) -> axum::Router {
        sccp_ingress_test_router_with_limit(app, usize::MAX)
    }

    fn sccp_ingress_echo_router_with_limit(
        app: SharedAppState,
        operator_max_body_bytes: usize,
    ) -> axum::Router {
        use axum::{Router, routing::post};

        seed_sccp_ingress_auth_account(&app);

        Router::new()
            .route(
                "/v1/bridge/proofs/submit",
                post(
                    |axum::Extension(admission): axum::Extension<
                        Arc<super::SccpSubmitAdmission>,
                    >,
                     body: axum::body::Bytes| async move {
                        admission.take().expect("take SCCP test admission");
                        body
                    },
                ),
            )
            .route(
                "/v1/bridge/messages",
                post(
                    |axum::Extension(admission): axum::Extension<
                        Arc<super::SccpSubmitAdmission>,
                    >,
                     body: axum::body::Bytes| async move {
                        admission.take().expect("take SCCP test admission");
                        body
                    },
                ),
            )
            .layer(axum::extract::DefaultBodyLimit::disable())
            .layer(axum::middleware::from_fn_with_state(
                super::SccpSubmitIngressState {
                    app,
                    operator_max_body_bytes,
                },
                super::enforce_sccp_submit_ingress,
            ))
    }

    fn sccp_body_that_must_not_be_polled() -> axum::body::Body {
        use std::task::Poll;

        let stream = futures::stream::poll_fn(
            |_context| -> Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
                panic!("rejected SCCP ingress polled the request body")
            },
        );
        axum::body::Body::from_stream(stream)
    }

    fn sccp_ingress_request(
        path: &str,
        body: axum::body::Body,
    ) -> axum::http::Request<axum::body::Body> {
        let mut request = axum::http::Request::builder()
            .method(axum::http::Method::POST)
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(body)
            .expect("valid SCCP ingress request");
        request
            .extensions_mut()
            .insert(crate::loopback_connect_info());
        request
    }
