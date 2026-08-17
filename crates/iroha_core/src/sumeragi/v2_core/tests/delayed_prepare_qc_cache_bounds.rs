#[test]
fn delayed_lower_prepare_qc_cannot_downgrade_retransmitted_progress() {
    let context = context();
    let high_subject = Subject::repeat(0x7e);
    let old_subject = Subject::repeat(0x7f);
    let mut reducer = Reducer::recover(
        context.clone(),
        Some(id(1)),
        Generation::new(31),
        [
            WalEntry::new(
                PersistenceId::new(1),
                WalRecord::InstallTimeout(tc_without_high(&context, 0, &[1, 2, 3])),
            ),
            WalEntry::new(
                PersistenceId::new(2),
                WalRecord::InstallTimeout(tc_without_high(&context, 1, &[1, 2, 3])),
            ),
        ],
    )
    .expect("recover at view two");
    assert_eq!(reducer.current_tag().view(), 2);
    let resumed = resume_after_replay(&mut reducer);
    assert_eq!(resumed.disposition(), StepDisposition::Applied);
    assert!(resumed.effects().is_empty());
    let higher = qc(&context, 1, Phase::Prepare, high_subject, &[1, 2, 3]);
    let persist_high = only_persist(
        reducer
            .step(Event::QuorumCertificateReceived {
                tag: reducer.current_tag(),
                certificate: higher.clone(),
            })
            .expect("observe high PrepareQC"),
    );
    acknowledge(&mut reducer, &persist_high);
    assert_eq!(
        reducer.volatile_prepare_counts(),
        (0, 1),
        "an old-view durable high QC has no pending body-pipeline owner"
    );
    let older = qc(&context, 0, Phase::Prepare, old_subject, &[1, 2, 3]);
    let before_older = reducer.clone();
    let ignored = reducer
        .step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: older,
        })
        .expect("an old PrepareQC is valid but cannot regress progress");
    assert_eq!(
        ignored.disposition(),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(ignored.effects().is_empty());
    assert_eq!(reducer, before_older);
    assert_eq!(reducer.volatile_prepare_counts(), (0, 1));
    let retry = reducer
        .step(Event::RetransmitElapsed {
            tag: reducer.current_tag(),
        })
        .expect("retransmit cached controls");
    let retained_prepare_qcs = retry
        .effects()
        .iter()
        .filter_map(|effect| match effect {
            Effect::Broadcast(ConsensusMessageV2::QuorumCertificate(certificate))
                if certificate.phase() == Phase::Prepare =>
            {
                Some(certificate)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(retained_prepare_qcs, vec![&higher]);
}
