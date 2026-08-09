#[test]
fn deferred_adapter_activation_marker_survives_a_no_progress_publication() {
    let _guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let directory = TempDir::new().expect("temporary directory");
    let context = context();
    let (mut adapter, startup) = SumeragiV2Adapter::open_deferred_status(
        directory.path().join("deferred-status.wal"),
        verified_genesis(context.clone()),
        None,
        reducer::Generation::new(context.height),
        [0xA6; 32],
        AdapterFingerprints {
            node: Hash::new(b"deferred node"),
            build: Hash::new(b"deferred build"),
            config: Hash::new(b"deferred config"),
        },
        DeferredAdmissionOrdinalSource::new(1),
    )
    .expect("open replayed adapter without status publication");

    assert!(startup.is_empty());
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "successor replay must remain invisible while its remaining constructors are fallible"
    );
    let prepared = adapter
        .successor_activation_status()
        .expect("prepare reducer-owned activation snapshot");
    assert_eq!(prepared.height, context.height);
    assert!(matches!(
        prepared.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            ..
        })
    ));
    assert!(
        crate::sumeragi::status::v2_status().is_none(),
        "preparing a snapshot is not publication"
    );
    crate::sumeragi::status::set_v2_status(prepared);

    let stale_tag = reducer::EventTag::new(
        context.height,
        0,
        reducer::Generation::new(context.height.saturating_sub(1)),
    );
    let ignored = adapter
        .retransmit_elapsed(stale_tag)
        .expect("publish an ignored post-activation retransmission");
    assert_eq!(
        ignored.disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::StaleGeneration)
    );
    let republished = crate::sumeragi::status::v2_status().expect("republished status");
    assert!(matches!(
        republished.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            ..
        })
    ));
    crate::sumeragi::status::clear_v2_status();
}

#[test]
fn executable_leader_rotation_matches_the_canonical_wire_context() {
    let wire_context = context();
    let mut registry = WireRegistry::new(&wire_context).expect("wire registry");
    let core_context = registry
        .core_context(&wire_context)
        .expect("executable context");

    for view in 0..=100 {
        let wire_leader = wire_context.leader(view);
        assert_eq!(
            registry
                .validator_index(core_context.leader(view))
                .expect("core leader maps to wire roster"),
            wire_leader,
            "leader mismatch in view {view}"
        );
    }
}

#[test]
fn successor_core_context_preserves_the_parent_certificate_binding() {
    let parent_context = context();
    let parent_round = wire::ConsensusRound {
        context_id: parent_context.id(),
        height: parent_context.height,
        view: 3,
    };
    let parent_qc = wire::QuorumCertificate {
        round: parent_round,
        proposal_round: parent_round,
        phase: wire::GlobalPhase::Commit,
        subject: subject(0x6d),
        execution_commitment: execution_commitment(0x6d),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x6d; 48],
    };
    let mut successor = parent_context.clone();
    successor.height += 1;
    successor.parent_commit_qc = Some(parent_qc);
    successor.validate().expect("structural successor context");
    let successor_id = successor.id();

    let mut registry = WireRegistry::new(&successor).expect("successor wire registry");
    let core_context = registry
        .core_context(&successor)
        .expect("parent-bound successor context");
    let core_parent = core_context
        .parent_commit()
        .expect("successor retains its parent CommitQC");

    assert_eq!(core_parent.context_id(), context_id(parent_context.id()));
    assert_ne!(core_parent.context_id(), context_id(successor_id));
    assert_eq!(core_parent.round().height(), parent_context.height);
    assert_eq!(core_parent.proposal_round().view(), parent_round.view);

    let parent_reference = successor
        .parent_commit_qc
        .as_ref()
        .expect("successor parent CommitQC")
        .as_ref();
    assert!(matches!(
        registry.qc_reference_to_core(&parent_reference),
        Err(AdapterError::WireValidation(
            wire::ValidationError::WrongHeightContext
        ))
    ));
}
