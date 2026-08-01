// Exact-replay coverage for prepared reports and partially completed PoR sink pipelines.

#[test]
fn scheduler_replays_exact_prepared_report_after_restart() {
    let dir = tempdir().expect("temp dir");
    let snapshot_path = canonical_temp_root(&dir).join("por_snapshot.to");
    let current_cycle = PorReportIsoWeek {
        year: 2025,
        week: 13,
    };
    let (current_start, _) = iso_week_bounds(current_cycle).expect("current ISO week bounds");
    let now_secs = u64::try_from(current_start.unix_timestamp()).expect("positive timestamp") + 60;
    let challenge = sample_challenge(true);
    let randomness = PorRandomness {
        epoch_id: challenge.epoch_id,
        issued_at_unix: challenge.issued_at,
        response_window_secs: challenge.deadline_at - challenge.issued_at,
        drand_round: challenge.drand_round,
        drand_randomness: challenge.drand_randomness,
        drand_signature: challenge.drand_signature,
    };
    let storage = Arc::new(ReplaySafeStorage {
        planned: Vec::new(),
        recorded: Arc::new(Mutex::new(None)),
    });
    let publisher = Arc::new(FailOnceWeeklyPublisher {
        attempts: AtomicUsize::new(0),
        reports: Mutex::new(Vec::new()),
    });

    {
        let coordinator =
            Arc::new(PorCoordinator::with_persistence(&snapshot_path).expect("coordinator"));
        let runtime = PorCoordinatorRuntime::new_with_publisher(
            storage.clone(),
            coordinator,
            Arc::new(StaticRandomnessProvider { randomness }),
            Arc::new(StaticVrfProvider::default()),
            publisher.clone(),
            3_600,
            900,
            300,
        );
        assert!(matches!(
            runtime.publish_weekly_report_if_needed(now_secs),
            Err(PorAutomationError::Governance(_))
        ));
    }

    let coordinator = Arc::new(PorCoordinator::with_persistence(&snapshot_path).expect("reload"));
    let runtime = PorCoordinatorRuntime::new_with_publisher(
        storage,
        Arc::clone(&coordinator),
        Arc::new(StaticRandomnessProvider { randomness }),
        Arc::new(StaticVrfProvider::default()),
        publisher.clone(),
        3_600,
        900,
        300,
    );
    runtime
        .publish_weekly_report_if_needed(now_secs)
        .expect("exact prepared report retry succeeds");
    runtime
        .publish_weekly_report_if_needed(now_secs)
        .expect("same cycle is already acknowledged");

    let reports = publisher.reports.lock();
    assert_eq!(reports.len(), 2);
    assert_eq!(
        norito::to_bytes(&reports[0]).expect("encode first attempt"),
        norito::to_bytes(&reports[1]).expect("encode retry")
    );
    assert_eq!(
        reports[1].cycle,
        PorReportIsoWeek {
            year: 2025,
            week: 12,
        }
    );
    assert_eq!(publisher.attempts.load(AtomicOrdering::SeqCst), 2);
    assert!(
        coordinator
            .prepared_weekly_report
            .read()
            .as_ref()
            .is_some_and(|prepared| prepared.published)
    );
}

#[tokio::test]
async fn runtime_retries_exact_sinks_after_mid_pipeline_failure() {
    let epoch_interval = 3_600;
    let epoch_id = 500_000;
    let vrf_deadline = 300;
    let now_secs = epoch_id * epoch_interval + vrf_deadline;
    assert_eq!(now_secs / epoch_interval, epoch_id);
    assert_eq!(now_secs % epoch_interval, vrf_deadline);
    let mut challenge = sample_challenge(true);
    challenge.epoch_id = epoch_id;
    challenge.issued_at = now_secs;
    challenge.deadline_at = now_secs + 900;
    challenge.seed = derive_challenge_seed(
        &challenge.drand_randomness,
        None,
        &challenge.manifest_digest,
        challenge.epoch_id,
    );
    challenge.challenge_id = derive_challenge_id(
        &challenge.seed,
        &challenge.manifest_digest,
        &challenge.provider_id,
        challenge.epoch_id,
        challenge.drand_round,
    );
    let planned = PlannedChallenge {
        challenge: challenge.clone(),
        duplicate_samples: 0,
    };
    let recorded = Arc::new(Mutex::new(None));
    let storage = Arc::new(ReplaySafeStorage {
        planned: vec![planned],
        recorded: Arc::clone(&recorded),
    });
    let publisher = Arc::new(FailOncePublisher {
        attempts: AtomicUsize::new(0),
        published: Mutex::new(Vec::new()),
    });
    let randomness = PorRandomness {
        epoch_id,
        issued_at_unix: now_secs,
        response_window_secs: 900,
        drand_round: challenge.drand_round,
        drand_randomness: challenge.drand_randomness,
        drand_signature: challenge.drand_signature,
    };
    let coordinator = Arc::new(PorCoordinator::new());
    let runtime = PorCoordinatorRuntime::new_with_publisher(
        storage,
        Arc::clone(&coordinator),
        Arc::new(StaticRandomnessProvider { randomness }),
        Arc::new(StaticVrfProvider::default()),
        publisher.clone(),
        epoch_interval,
        900,
        vrf_deadline,
    );

    assert!(matches!(
        runtime.run_once_at(now_secs).await,
        Err(PorAutomationError::Governance(_))
    ));
    assert_eq!(recorded.lock().as_ref(), Some(&challenge));
    assert_eq!(
        coordinator
            .query_statuses(&PorStatusFilter::default(), None, None)
            .len(),
        1
    );

    assert!(
        runtime
            .run_once_at(now_secs)
            .await
            .expect("exact retry succeeds")
    );
    let published = publisher.published.lock();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].challenge, challenge);
    assert_eq!(published[0].duplicate_samples, 0);
    drop(published);
    assert!(!runtime.run_once_at(now_secs).await.expect("epoch complete"));
    assert_eq!(publisher.attempts.load(AtomicOrdering::SeqCst), 2);
}
