    #[test]
    fn fair_v2_ingress_ownership_projection_ignores_route_liveness_until_maintenance() {
        let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
        let source = validator_peers(1).pop().expect("validator fixture");
        let semantic_origin = PeerId::from(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::new(source.clone());
        let initial_route = routes.mint_via(semantic_origin.clone(), source.clone());
        let request = v2_auxiliary_prepare(0);
        ingress.close();
        ingress
            .configure_roster([source.clone()])
            .expect("validator and anonymous lanes fit");
        ingress.open().expect("open configured roster");

        let inbound = |route: NetworkReplyRoute| {
            InboundBlockMessage::try_from_transport_with_reply_route(
                request.clone(),
                semantic_origin.clone(),
                source.clone(),
                route,
            )
            .expect("test route binds the semantic request and authenticated source")
        };
        assert!(matches!(
            ingress.try_push(inbound(initial_route.clone())),
            Ok(super::FairV2IngressPushDisposition::Enqueued)
        ));

        let admitted = {
            let state = ingress.state.lock();
            state
                .lanes
                .get(&super::FairV2IngressSource::Validator(source.clone()))
                .and_then(|lane| lane.entries.front())
                .and_then(|entry| entry.inbound.ingress_ownership.as_ref())
                .expect("fair admission attached ownership evidence")
                .clone()
        };
        assert!(admitted.validate_exact());
        let admitted_projection = admitted.process_local_projection_hash();

        assert!(routes.retire(&initial_route));
        assert!(!initial_route.is_active());
        assert!(admitted.validate_exact());
        assert_eq!(
            admitted.process_local_projection_hash(),
            admitted_projection,
            "transport cancellation cannot mutate immutable admission identity"
        );

        let mut projected_routes = admitted
            .current_reply_routes()
            .expect("admitted request retains its reply route")
            .clone();
        let mut maintained = admitted.clone();
        let (retained, prune_receipt) = projected_routes.retain_active_with_receipt();
        assert_eq!(retained, 0);
        projected_routes = maintained
            .project_retained_reply_routes(prune_receipt)
            .expect("authoritative pruning receipt updates the ownership carrier");
        assert!(projected_routes.is_empty());
        assert!(maintained.validate_exact());
        assert_ne!(
            maintained.process_local_projection_hash(),
            admitted_projection,
            "explicit route pruning must remain visible in the ownership projection"
        );

        let reconnect = routes.mint_via(semantic_origin.clone(), source.clone());
        assert!(matches!(
            ingress.try_push(inbound(reconnect.clone())),
            Ok(super::FairV2IngressPushDisposition::Coalesced)
        ));
        let delivered = ingress
            .try_recv()
            .expect("reconnected request retains the queued semantic owner");
        let reconnected = delivered
            .ingress_ownership()
            .expect("reconnected owner retains exact evidence");
        assert!(reconnected.validate_exact());
        assert_eq!(
            reconnected.latest_action(),
            super::FairV2IngressOwnershipAction::Reconnect
        );
        assert!(
            reconnected
                .current_reply_routes()
                .is_some_and(|retained| retained
                    .iter()
                    .any(|route| route.same_delivery(&reconnect)))
        );
    }

