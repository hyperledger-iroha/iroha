// Runtime/restart coverage for node-authoritative PoR projection replay.

#[test]
fn node_checkpoint_generation_rejects_pre_mutation_cursor_after_restart() {
    fn challenge_for(provider: u8, issued_at_offset: u64) -> PorChallengeV1 {
        let mut challenge = sample_challenge(false);
        challenge.provider_id = [provider; 32];
        challenge.issued_at += issued_at_offset;
        challenge.deadline_at += issued_at_offset;
        challenge.challenge_id = derive_challenge_id(
            &challenge.seed,
            &challenge.manifest_digest,
            &challenge.provider_id,
            challenge.epoch_id,
            challenge.drand_round,
        );
        challenge
    }

    let dir = tempdir().expect("temp dir");
    let config = sorafs_node::config::StorageConfig::builder()
        .enabled(true)
        .data_dir(canonical_temp_root(&dir).join("storage"))
        .build();
    let node = sorafs_node::NodeHandle::new(config.clone());
    for challenge in [challenge_for(0x91, 0), challenge_for(0x92, 1)] {
        node.record_por_challenge_with_authority_update(&challenge)
            .expect("commit initial node-authoritative challenge");
    }

    let limits =
        PorStatusPageLimits::new(1, POR_STATUS_PAGE_MAX_CANONICAL_BYTES_V1).expect("page limits");
    let coordinator = PorCoordinator::with_record_limit(64);
    coordinator
        .install_authoritative_projection(
            node.por_status_authority_snapshot()
                .expect("load node-authoritative projection"),
        )
        .expect("install node-authoritative projection");
    let first = coordinator
        .query_status_page(
            &PorStatusFilter::default(),
            limits,
            PorStatusPageCursor::First,
        )
        .expect("query first node-authoritative page");
    let cursor = first
        .next_cursor
        .expect("two statuses produce one continuation cursor");
    let issued_generation = first.snapshot_generation;

    node.record_por_challenge_with_authority_update(&challenge_for(0x93, 2))
        .expect("commit generation-advancing node mutation");
    drop(node);

    let restarted = sorafs_node::NodeHandle::new(config);
    let restored = restarted
        .por_status_authority_snapshot()
        .expect("restart restores the authoritative node checkpoint");
    assert!(restored.generation > issued_generation);
    let coordinator = PorCoordinator::with_record_limit(64);
    coordinator
        .install_authoritative_projection(restored.clone())
        .expect("install restored node-authoritative projection");
    let error = coordinator
        .query_status_page(
            &PorStatusFilter::default(),
            limits,
            PorStatusPageCursor::from_opaque(Some(&cursor)).expect("decode old cursor"),
        )
        .expect_err("pre-mutation cursor must stay stale after node restart");
    assert!(matches!(
        error,
        PorCoordinatorError::StalePageGeneration { expected, current }
            if expected == issued_generation && current == restored.generation
    ));
}
