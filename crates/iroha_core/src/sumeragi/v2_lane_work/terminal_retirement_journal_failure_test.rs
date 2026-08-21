#[test]
fn terminal_retirement_journal_failure_latches_lane_restart_with_gate_unchanged() {
    let CertifiedSidecarServerFixture {
        mut adapter,
        kura,
        requester,
        request,
        ..
    } = certified_sidecar_server_fixture();
    kura.remove_pending_certified_merge_entry(request.entry_hash)
        .expect("remove the sidecar before terminal materialization");
    adapter
        .merge_sidecars
        .obstruct_next_terminal_retirement_persist_for_test();
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
        hub.clone(),
        adapter.limits.reply_source_capacity.get(),
    );
    let reply_route = routes.mint_via(requester.clone(), hub);
    let output_guard = Arc::clone(&adapter.output_guard);
    assert_eq!(
        adapter.accept_relay_message(
            LaneRelayMessage::CertifiedMergeSidecar {
                sender: requester.clone(),
                reply_route: Some(reply_route),
                message: CertifiedMergeSidecarMessage::Request(request.clone()),
            },
            0,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert!(output_guard.restart_required());
    assert!(output_guard.acquire().is_none());
    assert!(
        adapter
            .merge_sidecars
            .has_server_request_gate_for_test(&requester, &request),
        "failed terminal persistence must leave the admitted gate in memory"
    );
    assert!(adapter.sidecar_effects.is_empty());
}
