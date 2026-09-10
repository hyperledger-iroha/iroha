#[derive(Clone, Copy, Debug)]
enum RetainedFinalityFixture {
    Chain,
    Standalone,
    Released,
}

fn retained_prepare_finality_fixture(
    shape: RetainedFinalityFixture,
    corrupt_prepare_signature: bool,
) -> (
    LifecycleLedgerV1,
    crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority,
) {
    let fixture = RecoveryFixture::new("retained-prepare-complete-tip", 0x51);
    let directory = TempDir::new().expect("stored signed Prepare body");
    let mut body_store = fixture.open_store(&directory);
    let original = fixture.fetch_record(&mut body_store, 0, 0x61, 2, None, false);
    let (cases, commit) =
        super::super::super::replay_authority::exact_retained_prepare_apply_family_fixture(
            &fixture.verified,
            &original.replay_authority,
            &fixture.keys,
            matches!(shape, RetainedFinalityFixture::Released),
            corrupt_prepare_signature,
        );
    let original_root = original.owner().causal_root();
    let first = if matches!(shape, RetainedFinalityFixture::Chain) {
        2
    } else {
        9
    };
    let original_owner = OwnerId::new(original_root, first);
    let apply_owner = if matches!(shape, RetainedFinalityFixture::Released) {
        OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xC9; 32])), 14)
    } else {
        original_owner
    };
    let ordinals = [2, 5, 9, 14];
    let mut records = Vec::new();
    for (index, case) in cases.into_iter().enumerate() {
        if index < 2 && !matches!(shape, RetainedFinalityFixture::Chain) {
            continue;
        }
        let continuation = match index {
            0 => DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 5),
            1 => DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 9),
            2 if matches!(shape, RetainedFinalityFixture::Released) => {
                DurableContinuation::AdvancedNoSuccessor
            }
            2 => DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 14),
            3 => DurableContinuation::None,
            _ => unreachable!(),
        };
        let owner = if index == 3 {
            apply_owner
        } else {
            original_owner
        };
        records.push(
            LifecycleLedgerRecordV1::new(
                case.key,
                owner,
                ordinals[index],
                case.work_class,
                case.stage,
                (index != 3).then_some(TerminalOutcome::Advanced),
                owner.causal_root().digest(),
                case.payload,
                case.authority,
                continuation,
            )
            .expect("original immutable body prefix and current live Apply"),
        );
    }
    let ledger = LifecycleLedgerV1::new(fixture.lifecycle_context(), 14, records, BTreeMap::new())
        .expect("structurally exact retained Prepare lineage");
    let artifact = wire::finality::V2FinalityArtifact::new(
        fixture.verified.context().clone(),
        commit.subject,
        commit,
        fixture.verified.proofs_of_possession().to_vec(),
    );
    let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
    let predecessor = crate::sumeragi::v2_recovery::DurableV2PredecessorIdentity::authenticate(
        &artifact, &receipt,
    )
    .expect("real signed Commit matches finality receipt");
    let successor = wire::HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
        b"retained Prepare finality successor",
    )));
    let activation = crate::sumeragi::v2_recovery::DurableSuccessorActivationAuthority::for_test(
        predecessor,
        successor,
    );
    let complete_tip = crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority::authenticate_for_test(
        artifact, receipt, successor, activation,
    ).expect("authenticated current CompleteTip");
    (ledger, complete_tip)
}

#[test]
fn complete_tip_preserves_historical_prepare_owners_through_terminal_recovery() {
    for shape in [
        RetainedFinalityFixture::Chain,
        RetainedFinalityFixture::Standalone,
        RetainedFinalityFixture::Released,
    ] {
        let (live, complete_tip) = retained_prepare_finality_fixture(shape, false);
        let original = live.encode();
        let prefix = live.records[..live.records.len() - 1].to_vec();
        let original_apply = live.records.last().expect("one Apply");
        let (terminal, changed, evidence) = live
            .stage_complete_tip_terminal_apply_recovery(&complete_tip, None)
            .unwrap_or_else(|error| panic!("recover {shape:?} finality: {error}"));
        assert!(changed);
        assert!(matches!(
            evidence,
            CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(14)
        ));
        assert_eq!(
            &terminal.records[..terminal.records.len() - 1],
            prefix.as_slice()
        );
        let apply = terminal.records.last().expect("same Apply");
        assert_eq!(apply.ordinal(), original_apply.ordinal());
        assert_eq!(apply.owner(), original_apply.owner());
        assert_eq!(apply.replay_authority, original_apply.replay_authority);
        assert_eq!(apply.terminal(), Some(Some(TerminalOutcome::Advanced)));
        assert_eq!(
            terminal
                .authenticate_complete_tip_terminal_apply(&complete_tip)
                .expect("exact terminal join"),
            14
        );
        let (stutter, changed, evidence) = terminal
            .stage_complete_tip_terminal_apply_recovery(&complete_tip, None)
            .expect("repeat the completed finality cut");
        assert!(!changed);
        assert!(matches!(
            evidence,
            CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(14)
        ));
        assert_eq!(stutter.encode(), terminal.encode());
        assert_eq!(
            live.encode(),
            original,
            "the input crash prefix is immutable"
        );
    }
}

#[test]
fn complete_tip_rejects_corrupt_historical_prepare_before_terminalization() {
    for shape in [
        RetainedFinalityFixture::Chain,
        RetainedFinalityFixture::Standalone,
        RetainedFinalityFixture::Released,
    ] {
        let (live, complete_tip) = retained_prepare_finality_fixture(shape, true);
        let before = live.encode();
        assert!(
            live.stage_complete_tip_terminal_apply_recovery(&complete_tip, None)
                .is_err(),
            "{shape:?}: current valid Commit must not authenticate a corrupt retained Prepare source"
        );
        assert_eq!(live.encode(), before);
    }
}
