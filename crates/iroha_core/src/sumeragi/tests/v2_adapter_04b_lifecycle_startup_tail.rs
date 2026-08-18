#[test]
fn recovered_prepare_already_repaired_child_reopens_and_publishes() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let safety = TempDir::new().expect("repaired-child safety directory");
    let ledger = TempDir::new().expect("repaired-child ledger");
    let payload = TempDir::new().expect("repaired-child payload store");
    let body = TempDir::new().expect("repaired-child body store");
    let (startup, _vote, proposal, manifest, validated) = reopen_with_prepare_intent(&safety, 0xDD);
    let replay_proposal = proposal.clone();
    let replay_manifest = manifest.clone();
    let replay_validated = validated.clone();

    let mut first_holder =
        super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let first =
        join_recovered_prepare_startup(startup, proposal, manifest, validated, &mut first_holder);
    let (_summary, durable_before_crash) = first
        .persist_repair_for_test(ledger.path())
        .unwrap_or_else(|error| panic!("fsync the first repaired frame: {}", error.reason()));
    drop(durable_before_crash);

    let restarted = open_recovered_startup_test(&safety)
        .expect("fresh startup replays the unchanged repaired WAL frame");
    let mut restarted_holder =
        super::super::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
    let restarted = join_recovered_prepare_startup(
        restarted,
        replay_proposal,
        replay_manifest,
        replay_validated,
        &mut restarted_holder,
    );
    let (changed, durable) = restarted
        .persist_reopened_repair_for_test(ledger.path())
        .unwrap_or_else(|error| {
            panic!(
                "stutter on the already repaired ledger frame: {}",
                error.reason()
            )
        });
    assert!(!changed);
    let installed = durable
        .install_recovered_sign_for_test(ledger.path())
        .unwrap_or_else(|error| {
            panic!("install the repaired-frame Sign child: {}", error.reason())
        });
    let verified = verified_from_installed_startup(&installed);
    let (mut payload_store, mut recovery) = empty_authenticated_lifecycle_recovery(
        &verified,
        ledger.path(),
        payload.path(),
        body.path(),
    );
    assert!(
        installed
            .installed
            .seed_child_recovery_for_test(&mut recovery)
    );
    crate::sumeragi::status::clear_v2_status();
    let published = installed
        .open_coordinator_and_publish_for_test(ledger.path(), &mut payload_store, recovery)
        .unwrap_or_else(|error| {
            panic!(
                "already-repaired child must reopen idempotently: {}",
                error.reason()
            )
        });
    assert!(published.exact_published_join_for_test());
    assert!(crate::sumeragi::status::v2_status().is_some());
    drop(published);
    crate::sumeragi::status::clear_v2_status();
}

#[cfg(feature = "bls")]
#[test]
fn recovered_commit_vote_sign_retains_the_exact_authenticated_prepare_qc() {
    let directory = TempDir::new().expect("temporary Commit recovery directory");
    let (mut adapter, startup) = open_test(&directory).expect("open Commit replay fixture");
    assert!(startup.is_empty());
    let locked_subject = subject(0xD2);
    let round = wire::ConsensusRound {
        context_id: adapter.wire_context.id(),
        height: adapter.wire_context.height,
        view: 0,
    };
    let (_, keys, _) = authenticated_context();
    let mut wire_prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: locked_subject,
        execution_commitment: execution_commitment(0xD2),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut wire_prepare, &keys);
    let prepare = adapter
        .registry
        .qc_to_core(&wire_prepare, &adapter.wire_context)
        .expect("register the authenticated PrepareQC");
    let core_context = adapter.reducer.context().id();
    let core_round = reducer::Round::new(round.height, round.view);
    let core_subject = prepare.subject();
    let local_validator = adapter
        .registry
        .validator_id(0)
        .expect("fixture local validator");
    let entry = reducer::WalEntry::new(
        reducer::PersistenceId::new(1),
        reducer::WalRecord::LockAndCommit {
            prepare,
            vote: reducer::Vote::new_with_proposal_round(
                core_context,
                core_round,
                core_round,
                reducer::Phase::Commit,
                core_subject,
                local_validator,
            ),
        },
    );
    let encoded = adapter
        .registry
        .encode_wal_entry(&entry, &TestAggregator)
        .expect("encode the exact LockAndCommit frame");
    assert_eq!(
        adapter
            .wal
            .append(&encoded)
            .expect("append lock frame")
            .sequence(),
        0
    );
    drop(adapter);

    let startup = open_recovered_startup_test(&directory)
        .expect("replay authenticated LockAndCommit behind the sealed startup cut");
    let authenticated = match startup.authenticate_final_wal_startup_authority() {
        Ok(authenticated) => authenticated,
        Err((error, _startup)) => {
            panic!("authenticate the current recovered LockAndCommit: {error}")
        }
    };
    let authority = authenticated
        .recovered_phase_vote_for_test()
        .expect("LockAndCommit carries one restart vote");
    assert!(authenticated.effects.is_empty());
    assert!(authority.wal_identity().is_exact());
    assert!(authority.replay_evidence_is_exact());
    assert!(
        authority.exactly_matches_wal_record(
            authenticated
                .adapter
                .wal
                .recovered_records()
                .last()
                .expect("LockAndCommit WAL frame remains retained")
        )
    );
    assert_eq!(authority.vote().phase, wire::GlobalPhase::Commit);
    assert_eq!(authority.vote().round, round);
    assert_eq!(authority.vote().proposal_round, round);
    assert_eq!(authority.vote().subject, locked_subject);
    let retained_prepare = authority
        .prepare_certificate()
        .expect("Commit recovery retains the exact PrepareQC");
    assert_eq!(retained_prepare, &wire_prepare);
    assert_eq!(
        retained_prepare.execution_commitment,
        authority.vote().execution_commitment
    );
    drop(authenticated);
}

#[test]
fn recovered_vote_sign_startup_cut_is_one_shot_and_drop_inert() {
    let directory = TempDir::new().expect("temporary recovery seal directory");
    let (startup, _expected_vote, _proposal, _manifest, _validated) =
        reopen_with_prepare_intent(&directory, 0xD3);
    let wal_path = directory.path().join("safety.wal");
    let durable_before = std::fs::read(&wal_path).expect("read sealed WAL before drop");
    let authenticated = match startup.authenticate_final_wal_startup_authority() {
        Ok(authenticated) => authenticated,
        Err((error, _startup)) => panic!("authenticate exact replay vote: {error}"),
    };
    assert!(authenticated.recovered_phase_vote_for_test().is_some());
    assert!(
        authenticated.effects.iter().all(|effect| !matches!(
            effect,
            AdapterEffect::Sign {
                request: SignRequest::Vote(_),
                ..
            }
        )),
        "the retained batch no longer contains the sealed vote"
    );
    assert!(
        authenticated.finish_without_wal_vote().is_err(),
        "a phase-vote startup cannot escape through the no-vote path"
    );
    assert_eq!(
        std::fs::read(&wal_path).expect("read sealed WAL after drop"),
        durable_before,
        "dropping the sealed startup cannot rewrite its WAL"
    );

    let repeated = open_recovered_startup_test(&directory)
        .expect("the unchanged WAL can be authenticated by a new sealed startup instance");
    let repeated = repeated
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("reauthenticate unchanged WAL: {error}"));
    assert!(repeated.recovered_phase_vote_for_test().is_some());
    assert!(repeated.effects.is_empty());
    drop(repeated);
}

#[test]
fn recovered_startup_seals_authenticated_control_wal_records() {
    let directory = TempDir::new().expect("temporary non-vote recovery directory");
    let (mut adapter, startup) = open_test(&directory).expect("open timeout replay fixture");
    assert!(startup.is_empty());
    let timeout = adapter
        .timeout_elapsed(adapter.current_tag())
        .expect("persist the exact TimeoutIntent")
        .into_effects();
    assert!(matches!(
        timeout.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    drop(adapter);

    let startup = open_recovered_startup_test(&directory)
        .expect("replay the durable TimeoutIntent behind the sealed startup cut");
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| panic!("authenticate TimeoutIntent: {error}"));
    assert!(authenticated.has_recovered_control_sign_for_test());
    assert!(authenticated.effects.is_empty());
    assert!(
        authenticated.finish_without_wal_vote().is_err(),
        "a control Sign cannot escape through the no-authority path"
    );
}
