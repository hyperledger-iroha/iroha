// Shared-kernel cases for a local height rooted in external finalized state.
// Cryptographic/state-proof authentication belongs to the native adapter;
// these tests exercise the production reducer and its logical WAL boundary.

fn finalized_state_anchor_fixture() -> FinalizedStateAnchor {
    FinalizedStateAnchor {
        context_id: ContextId::repeat(0xa1),
        height: 41,
        subject: Subject::repeat(0xa2),
        predecessor_height: 1,
        predecessor_subject: Some(Subject::repeat(0xa3)),
    }
}

fn context_from_finalized_state(
    anchor: FinalizedStateAnchor,
    height: u64,
    roster_len: u8,
) -> Result<HeightContext, HeightContextError> {
    HeightContext::new_from_finalized_state(
        ContextId::repeat(0xb0),
        NetworkId::repeat(0xb1),
        height,
        anchor,
        7,
        (1..=roster_len)
            .map(|validator| Validator::new(id(validator), VotingPower::new(1)))
            .collect(),
        VotingMode::Permissioned,
        Digest::repeat(0xb2),
        Digest::repeat(0xb3),
        Digest::repeat(0xb4),
        Digest::default(),
    )
}

#[test]
fn finalized_state_anchor_is_explicit_and_does_not_relax_ordinary_parent_rules() {
    let anchor = finalized_state_anchor_fixture();
    let context = context_from_finalized_state(anchor, 2, 4).expect("exact external anchor");
    assert_eq!(context.finalized_state_anchor(), Some(anchor));
    assert_eq!(context.parent_commit(), None);
    assert!(!context.is_snapshot_bootstrap());
    assert_eq!(context.minimum_signer_count(), 3);
    assert_eq!(
        HeightContext::new(
            context.id(),
            context.network_id(),
            context.height(),
            None,
            context.epoch(),
            context.roster().to_vec(),
            context.mode(),
            context.nexus_amx_context_hash(),
            context.execution_policy_hash(),
            context.da_layout_hash(),
            Digest::default(),
        ),
        Err(HeightContextError::InvalidParentCommit),
        "an absent parent needs explicit authenticated external authority"
    );
    let empty = FinalizedStateAnchor {
        predecessor_height: 0,
        predecessor_subject: None,
        ..anchor
    };
    assert!(context_from_finalized_state(empty, 1, 4).is_ok());
    assert_eq!(
        context_from_finalized_state(anchor, 2, 5),
        Err(HeightContextError::InvalidCommitteeGeometry)
    );
}

#[test]
fn finalized_state_anchor_rejects_zero_and_noncontiguous_authority() {
    let valid = finalized_state_anchor_fixture();
    for invalid in [
        FinalizedStateAnchor {
            context_id: ContextId::default(),
            ..valid
        },
        FinalizedStateAnchor { height: 0, ..valid },
        FinalizedStateAnchor {
            subject: Subject::default(),
            ..valid
        },
        FinalizedStateAnchor {
            predecessor_height: 0,
            ..valid
        },
        FinalizedStateAnchor {
            predecessor_height: 2,
            ..valid
        },
        FinalizedStateAnchor {
            predecessor_height: u64::MAX,
            ..valid
        },
        FinalizedStateAnchor {
            predecessor_subject: None,
            ..valid
        },
        FinalizedStateAnchor {
            predecessor_subject: Some(Subject::default()),
            ..valid
        },
    ] {
        assert_eq!(
            context_from_finalized_state(invalid, 2, 4),
            Err(HeightContextError::InvalidFinalizedStateAnchor)
        );
    }
    for height in [0, 1, 3, u64::MAX] {
        assert_eq!(
            context_from_finalized_state(valid, height, 4),
            Err(HeightContextError::InvalidFinalizedStateAnchor)
        );
    }
}

#[test]
fn finalized_state_anchor_silent_author_rotates_only_after_durable_timeout_quorum() {
    for roster_len in [4, 7] {
        let context = context_from_finalized_state(finalized_state_anchor_fixture(), 2, roster_len)
            .expect("exact external anchor");
        let local = context.leader(1);
        let mut reducer = Reducer::new(context.clone(), Some(local), Generation::INITIAL)
            .expect("survivor opens the local height without any payload");
        assert_ne!(local, context.leader(0));
        let timeout = only_persist(
            reducer
                .step(Event::TimeoutElapsed {
                    tag: reducer.current_tag(),
                })
                .expect("silent author can be timed out before a proposal"),
        );
        assert!(matches!(timeout.record(), WalRecord::TimeoutIntent(_)));
        assert_eq!(reducer.current_tag().view(), 0);
        assert!(reducer.awaiting_signature().is_none());
        let signed = acknowledge(&mut reducer, &timeout);
        assert!(matches!(
            signed.effects(),
            [Effect::Sign {
                message: SignableMessage::TimeoutVote(_),
                ..
            }]
        ));
        complete_signature(&mut reducer, 2);
        let quorum = context.minimum_signer_count();
        let survivors = (2..=roster_len).take(quorum).collect::<Vec<_>>();
        assert_eq!(survivors.len(), quorum);
        let insufficient = tc_without_high(&context, 0, &survivors[..quorum - 1]);
        assert!(insufficient.validate(&context).is_err());
        let certificate = tc_without_high(&context, 0, &survivors);
        let install = only_persist(
            reducer
                .step(Event::TimeoutCertificateReceived {
                    tag: reducer.current_tag(),
                    certificate,
                })
                .expect("quorum advances without any payload"),
        );
        assert_eq!(
            reducer.current_tag().view(),
            0,
            "unacknowledged TC grants no new leader"
        );
        let entered = acknowledge(&mut reducer, &install);
        assert_eq!(reducer.current_tag().view(), 1);
        assert!(
            entered
                .effects()
                .iter()
                .any(|effect| matches!(effect, Effect::EnterView { .. }))
        );
        let proposal = only_persist(
            reducer
                .step(Event::LocalProposalReady {
                    tag: reducer.current_tag(),
                    manifest: PayloadManifest::new(
                        Subject::repeat(0xc1),
                        Digest::repeat(0xc2),
                        Digest::repeat(0xc3),
                        128,
                        2,
                    ),
                })
                .expect("replacement leader can select a fresh body under an unlocked TC"),
        );
        assert!(matches!(proposal.record(), WalRecord::ProposalIntent(_)));
        let recovered = Reducer::recover(
            context.clone(),
            Some(local),
            Generation::INITIAL,
            [timeout, install, proposal],
        )
        .expect("the same external anchor reconstructs the durable successor");
        assert_eq!(recovered.current_tag().view(), 1);
        assert_eq!(
            recovered.context().finalized_state_anchor(),
            context.finalized_state_anchor()
        );
    }
}

#[test]
fn finalized_state_anchor_recovery_retains_timeout_lock_and_rejects_competing_body() {
    let context = context_from_finalized_state(finalized_state_anchor_fixture(), 2, 4)
        .expect("exact external anchor");
    let locked_subject = Subject::repeat(0xd1);
    let prepare = qc(&context, 0, Phase::Prepare, locked_subject, &[1, 2, 3]);
    let entry = WalEntry::new(
        PersistenceId::new(1),
        WalRecord::InstallTimeout(tc_with_high(&context, 0, prepare.clone(), &[2, 3, 4])),
    );
    let mut recovered = Reducer::recover(
        context.clone(),
        Some(context.leader(1)),
        Generation::INITIAL,
        [entry.clone()],
    )
    .expect("restart preserves the highest PrepareQC from a real logical WAL frame");
    assert_eq!(recovered.durable_state().locked(), Some(&prepare));
    assert_eq!(recovered.current_tag().view(), 1);
    resume_after_replay(&mut recovered);
    let wrong = recovered
        .step(Event::LocalProposalReady {
            tag: recovered.current_tag(),
            manifest: PayloadManifest::new(
                Subject::repeat(0xd2),
                Digest::repeat(0xd3),
                Digest::repeat(0xd4),
                128,
                2,
            ),
        })
        .expect("competing body is a rejected proposal, not a new signing intent");
    assert_eq!(
        wrong.disposition(),
        StepDisposition::Ignored(IgnoreReason::UnsafeProposal)
    );
    assert!(wrong.effects().is_empty());
    assert_eq!(recovered.durable_state().locked(), Some(&prepare));
    assert!(
        Reducer::recover(self::context(), Some(id(2)), Generation::INITIAL, [entry],).is_err(),
        "a foreign instance cannot replay the lane timeout authority"
    );
}
