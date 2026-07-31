
    #[test]
    fn exact_authenticated_timeout_certificate_from_distinct_sources_coalesces_in_one_runtime_slot()
    {
        let directory = TempDir::new().expect("temporary multi-source TC directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(4, 1, 1),
            Some(0),
        );
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before authenticated ingress");
        let round_tag = runtime.round_tag();
        let timeout_effects = runtime
            .driver
            .timeout_elapsed(round_tag)
            .expect("install a local signing fence")
            .into_effects();
        assert!(matches!(
            timeout_effects.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ));

        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
                signed_runtime_timeout_certificate(&context, &keys),
            ));
        let first_source = PeerId::new(keys[1].public_key().clone());
        let second_source = PeerId::new(keys[2].public_key().clone());
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, first_source),
                )
                .expect("the first authenticated TC carrier owns the runtime command"),
            round_tag
        );
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, second_source),
                )
                .expect("the same TC from another source coalesces"),
            round_tag
        );
        assert_eq!(
            runtime.queued_commands(),
            1,
            "one exact aggregate TC must retain every bounded source carrier"
        );
        let retained = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.as_ref())
            .expect("the coalesced TC retains exact ingress ownership");
        assert!(retained.validate_exact());
        assert_eq!(retained.direct.len(), 2);

        assert!(matches!(
            runtime.step(now),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("the Busy TC dispatch retains its exact runtime owner");
        assert!(selected.validate_exact().is_ok());
        let deferred = runtime
            .deferred_ingress_ownership
            .values()
            .next()
            .expect("the Busy TC retains the coalesced source carriers");
        assert!(deferred.validate_exact());
        assert_eq!(deferred.direct.len(), 2);
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn admitted_progress_runs_after_its_frozen_prefix_before_later_normal_churn() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        for value in 0..3 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .unwrap();
        }
        for value in 100..140 {
            assert_eq!(
                enqueue_fake(
                    &mut runtime,
                    initial,
                    CommandClass::Normal,
                    FakeCommand::record(value)
                ),
                Err(EnqueueError::ReservedCapacity)
            );
        }
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(200),
        )
        .expect("CommitQC/progress reserve remains available");

        let initial_queue = runtime.queue_snapshot(start);
        assert_eq!(initial_queue.normal.depth, 3);
        assert_eq!(initial_queue.progress.depth, 1);

        for (expected, replacement) in [(0, 3), (1, 4), (2, 5)] {
            runtime
                .step_and_take_scheduler_ownership_for_test(start)
                .expect("one frozen normal predecessor drains");
            assert_eq!(runtime.driver.delivered.last(), Some(&(initial, expected)));
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(replacement),
            )
            .expect("later normal churn may refill only the vacated normal slot");
        }
        runtime
            .step_and_take_scheduler_ownership_for_test(start)
            .expect("the admitted progress owner runs after its finite frozen prefix");
        assert_eq!(
            runtime.driver.delivered,
            vec![(initial, 0), (initial, 1), (initial, 2), (initial, 200)]
        );
        let queue = runtime.queue_snapshot(start);
        assert_eq!(queue.normal.depth, 3);
        assert_eq!(queue.normal.capacity, 3);
        assert_eq!(queue.normal.max_service_debt, 1);
        assert_eq!(queue.progress.depth, 0);
        assert_eq!(queue.completion.depth, 0);
    }

    #[test]
    fn periodic_retransmit_cannot_starve_admitted_work_when_every_step_arrives_late() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        for value in 1..=2 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .unwrap();
        }

        for seconds in [2, 4, 6, 8] {
            let _ = runtime
                .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(seconds));
        }

        assert_eq!(runtime.driver.retransmits, vec![initial, initial]);
        assert_eq!(runtime.driver.delivered, vec![(initial, 1), (initial, 2)]);
    }

    #[test]
    fn frozen_lifecycle_order_precedes_timeout_priority() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(7),
        )
        .unwrap();

        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2))
            .expect("the older admitted FIFO lifecycle dispatches first");
        assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
        assert!(runtime.driver.retransmits.is_empty());
        assert!(runtime.driver.timeouts.is_empty());

        runtime
            .step(start + Duration::from_secs(10))
            .expect("the earlier frozen periodic lifecycle dispatches next");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("periodic retransmit publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::PeriodicTimer
        );
        assert_eq!(runtime.driver.retransmits, vec![initial]);
        assert!(runtime.driver.timeouts.is_empty());

        runtime
            .step(start + Duration::from_secs(12))
            .expect("the later absolute-timeout lifecycle dispatches last");
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .expect("absolute timeout publishes scheduler ownership")
                .selected,
            RuntimeSelectedOwnerKind::Timeout
        );
        assert_eq!(runtime.driver.timeouts, vec![initial]);
        assert_eq!(
            runtime.driver.retransmits,
            vec![initial],
            "the absolute deadline cannot replenish the drained periodic owner"
        );
    }

    #[test]
    fn due_timeout_becomes_older_than_replenished_exact_serve_tickets() {
        let start = Instant::now();
        let initial = tag(0);
        let lifecycle_ordinals = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut runtime = SerializedV2Runtime::with_driver_and_lifecycle_ordinals(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(5, 1, 1),
            Vec::new(),
            lifecycle_ordinals.clone(),
        )
        .expect("construct runtime with the shared Serve source")
        .0;
        runtime
            .arm_live_clocks(start)
            .expect("arm shared-source runtime");

        let first_barrier = lifecycle_ordinals
            .reserve_one()
            .expect("reserve first exact Serve occurrence");
        assert!(
            !runtime
                .older_lifecycle_predates_exact_serve(
                    start + Duration::from_secs(10),
                    first_barrier,
                )
                .expect("first barrier freezes the due timeout"),
            "a clock first frozen behind this ticket cannot overtake it"
        );

        let second_barrier = lifecycle_ordinals
            .reserve_one()
            .expect("reserve a distinct retransmission occurrence");
        assert!(
            runtime
                .older_lifecycle_predates_exact_serve(
                    start + Duration::from_secs(10),
                    second_barrier,
                )
                .expect("replenished barrier validates against the same source"),
            "the frozen timeout must predate every later exact ticket"
        );
        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("one bounded predecessor episode dispatches the timeout");
        assert_eq!(runtime.driver.timeouts, vec![initial]);
    }

