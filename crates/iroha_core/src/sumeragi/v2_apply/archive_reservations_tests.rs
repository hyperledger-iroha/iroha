mod archive_reservations {
    use super::*;
    use crate::{
        query::{
            provider_ingest_finalized::{
                ProviderCandidateCapture, ProviderIngestFinalizedArchiveKeyV1,
            },
            reputation_finalized::{ReputationCandidateCapture, ReputationFinalizedArchiveKeyV1},
        },
        sumeragi::{
            v2_apply::archive_reservations::CandidateArchiveReservations,
            v2_body_store::LocalValidationRefusal,
        },
    };
    use std::{
        future::Future,
        pin::Pin,
        sync::atomic::Ordering,
        task::{Context, Poll},
    };

    struct Archives {
        provider: Arc<ProviderIngestFinalizedArchiveV1>,
        reputation: Arc<ReputationFinalizedArchive>,
        directory: tempfile::TempDir,
    }

    impl Archives {
        fn install(fixture: &mut ApplyFixture) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let provider = Arc::new(
                ProviderIngestFinalizedArchiveV1::try_open(
                    root.join("provider"),
                    ProviderIngestFinalizedArchiveBoundsV1::try_new(
                        1 << 20,
                        16,
                        16 << 20,
                        16,
                        16,
                        256,
                        16,
                    )
                    .unwrap(),
                )
                .unwrap(),
            );
            let reputation = Arc::new(
                ReputationFinalizedArchive::try_open(
                    root.join("reputation"),
                    ReputationFinalizedArchiveBounds::try_new(1 << 20, 16, 16 << 20).unwrap(),
                )
                .unwrap(),
            );
            fixture.service.provider_ingest_finalized_archive = Some(Arc::clone(&provider));
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&reputation));
            Self {
                provider,
                reputation,
                directory,
            }
        }

        fn provider_owner(&self, fixture: &ApplyFixture) -> ProviderCandidateCapture {
            self.provider
                .try_reserve_candidate(
                    ProviderIngestFinalizedArchiveKeyV1::try_new(
                        fixture.context.network_id,
                        fixture.context.height,
                        *fixture.body.hash().as_ref(),
                        fixture.body.header().creation_time_ms,
                    )
                    .unwrap(),
                    &fixture.kura,
                )
                .unwrap()
        }

        fn reputation_owner(&self, fixture: &ApplyFixture) -> ReputationCandidateCapture {
            self.reputation
                .try_reserve_candidate(
                    ReputationFinalizedArchiveKeyV1::try_new(
                        fixture.context.network_id,
                        fixture.context.height,
                        *fixture.body.hash().as_ref(),
                    )
                    .unwrap(),
                    fixture.body.header().creation_time_ms,
                    &fixture.kura,
                )
                .unwrap()
        }
    }

    fn reserve(fixture: &ApplyFixture) -> Result<CandidateArchiveReservations, V2ApplyError> {
        fixture
            .service
            .try_reserve_candidate_archives(&fixture.context, &fixture.body)
    }

    fn busy(error: V2ApplyError, resource: &'static str) -> BodyValidationBusy {
        assert!(error.rejection_identity().is_none());
        match error {
            V2ApplyError::LocalValidation(LocalValidationRefusal::PhysicalBusy(busy)) => {
                assert_eq!(busy.resource, resource);
                busy
            }
            error => panic!("expected original {resource} release, got {error:?}"),
        }
    }

    fn capture_count(fixture: &ApplyFixture) -> usize {
        fixture
            .service
            .test_failures
            .candidate_executions
            .load(Ordering::Relaxed)
    }

    #[test]
    fn acquires_original_pair_without_execution() {
        fn assert_send_static<T: Send + 'static>() {}
        assert_send_static::<CandidateArchiveReservations>();
        let mut fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let calls = capture_count(&fixture);
        let generation = fixture.state.state_view_generation();
        let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
        let empty = reserve(&fixture).unwrap();
        let (provider, reputation) = empty
            .into_captures(&fixture.service, &fixture.context, &fixture.body)
            .unwrap();
        assert!(provider.is_none() && reputation.is_none());
        let archives = Archives::install(&mut fixture);
        let owner = reserve(&fixture).unwrap();
        // Both original physical index readers can proceed while the logical
        // predecessor remains reserved and movable to the detached worker.
        archives.provider.with_index_reader_for_test(|| ());
        archives.reputation.with_index_reader_for_test(|| ());
        let (provider, reputation) = owner
            .into_captures(&fixture.service, &fixture.context, &fixture.body)
            .unwrap();
        assert!(provider.is_some() && reputation.is_some());
        drop((provider, reputation));
        drop(reserve(&fixture).unwrap());
        assert_eq!(capture_count(&fixture), calls);
        assert_eq!(fixture.state.state_view_generation(), generation);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(),
            before
        );
    }

    #[test]
    fn index_busy_wakes_original_runner() {
        for provider_busy in [true, false] {
            let mut fixture = ApplyFixture::new_for_production_recovered_decision_apply();
            let archives = Archives::install(&mut fixture);
            let calls = capture_count(&fixture);
            let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(1);
            fixture.service.queue.set_sumeragi_wake(wake_tx);
            let probe = || {
                let resource = if provider_busy {
                    "provider_archive_index"
                } else {
                    "reputation_archive_index"
                };
                let busy = busy(reserve(&fixture).unwrap_err(), resource);
                let mut wait = busy.wait.clone().wait_for_release();
                assert_eq!(
                    Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
                    Poll::Pending
                );
                assert!(wake_rx.try_recv().is_err());
                if !provider_busy {
                    // The successful first reservation was released before the
                    // service returned its second-index refusal.
                    drop(archives.provider_owner(&fixture));
                }
                (busy, wait)
            };
            let (busy, mut wait) = if provider_busy {
                archives.provider.with_index_reader_for_test(probe)
            } else {
                archives.reputation.with_index_reader_for_test(probe)
            };
            wake_rx
                .try_recv()
                .expect("actual index release wakes original runner");
            assert_eq!(
                Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
                Poll::Ready(())
            );
            drop(reserve(&fixture).unwrap());
            assert_eq!(capture_count(&fixture), calls);
        }
    }

    #[test]
    fn second_capture_refusal_releases_first() {
        let mut fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let archives = Archives::install(&mut fixture);
        let calls = capture_count(&fixture);
        let second = archives.reputation_owner(&fixture);
        let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(1);
        fixture.service.queue.set_sumeragi_wake(wake_tx);
        let busy = busy(reserve(&fixture).unwrap_err(), "reputation_archive_capture");
        drop(archives.provider_owner(&fixture));
        let mut wait = busy.wait.clone().wait_for_release();
        assert_eq!(
            Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
            Poll::Pending
        );
        assert!(wake_rx.try_recv().is_err());
        drop(second);
        wake_rx
            .try_recv()
            .expect("original second capture drop wakes runner");
        assert_eq!(
            Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
            Poll::Ready(())
        );
        drop(reserve(&fixture).unwrap());
        assert_eq!(capture_count(&fixture), calls);
    }

    #[test]
    fn original_capture_drop_wakes_runner_and_preserves_old_wait() {
        let mut fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let _archives = Archives::install(&mut fixture);
        let owner = reserve(&fixture).unwrap();
        let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(1);
        fixture.service.queue.set_sumeragi_wake(wake_tx);
        let busy = busy(reserve(&fixture).unwrap_err(), "provider_archive_capture");
        let mut wait = busy.wait.clone().wait_for_release();
        assert_eq!(
            Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
            Poll::Pending
        );
        let before_registration = busy.wait.clone();
        std::thread::spawn(move || drop(owner)).join().unwrap();
        wake_rx
            .try_recv()
            .expect("original owner drop wakes runner");
        let newer = reserve(&fixture).unwrap();
        assert_eq!(
            Pin::new(&mut wait).poll(&mut Context::from_waker(busy.waker())),
            Poll::Ready(())
        );
        let mut late = before_registration.wait_for_release();
        assert_eq!(
            Pin::new(&mut late).poll(&mut Context::from_waker(busy.waker())),
            Poll::Ready(())
        );
        let new_busy = busy_error(&fixture);
        let mut new_wait = new_busy.wait.clone().wait_for_release();
        assert_eq!(
            Pin::new(&mut new_wait).poll(&mut Context::from_waker(new_busy.waker())),
            Poll::Pending
        );
        drop(newer);
    }

    fn busy_error(fixture: &ApplyFixture) -> BodyValidationBusy {
        busy(reserve(fixture).unwrap_err(), "provider_archive_capture")
    }

    #[test]
    fn rejects_mismatch_before_acquisition() {
        let mut fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let archives = Archives::install(&mut fixture);
        let calls = capture_count(&fixture);
        for network_mismatch in [true, false] {
            let mut context = fixture.context.clone();
            if network_mismatch {
                context.network_id =
                    crate::sumeragi::synthetic_network_id("foreign archive network");
            } else {
                context.height += 1;
            }
            let error = fixture
                .service
                .try_reserve_candidate_archives(&context, &fixture.body)
                .unwrap_err();
            assert!(matches!(error, V2ApplyError::TaskMismatch));
            drop(archives.provider_owner(&fixture));
            drop(archives.reputation_owner(&fixture));
        }
        let original_kura = Arc::clone(&fixture.service.kura);
        fixture.service.kura = Kura::blank_kura_for_testing();
        let error = reserve(&fixture).unwrap_err();
        assert!(matches!(
            error,
            V2ApplyError::LocalValidation(LocalValidationRefusal::RecoveryRequired(_))
        ));
        fixture.service.kura = original_kura;
        drop(reserve(&fixture).unwrap());
        assert_eq!(capture_count(&fixture), calls);
    }

    #[test]
    fn handoff_retains_owner_on_context_wire_and_service_mismatch() {
        let mut fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let _archives = Archives::install(&mut fixture);
        let owner = reserve(&fixture).unwrap();
        let mut other_context = fixture.context.clone();
        other_context.leader_seed[0] ^= 1;
        let (owner, error) =
            match owner.into_captures(&fixture.service, &other_context, &fixture.body) {
                Err(refusal) => refusal,
                Ok(_) => panic!("context substitution consumed original reservations"),
            };
        assert!(matches!(error, V2ApplyError::TaskMismatch));
        let mut other_wire = fixture.body.clone();
        let signature = iroha_crypto::SignatureOf::try_from_hash(
            fixture.genesis_key.private_key(),
            other_wire.header().hash(),
        )
        .unwrap();
        other_wire
            .add_signature(iroha_data_model::block::BlockSignature::new(1, signature))
            .unwrap();
        assert_eq!(
            other_wire.hash(),
            fixture.body.hash(),
            "only canonical wire changed"
        );
        let (owner, error) =
            match owner.into_captures(&fixture.service, &fixture.context, &other_wire) {
                Err(refusal) => refusal,
                Ok(_) => panic!("same-header different-wire body consumed original reservations"),
            };
        assert!(matches!(error, V2ApplyError::TaskMismatch));
        let foreign = ApplyFixture::new_for_production_recovered_decision_apply();
        let (owner, error) =
            match owner.into_captures(&foreign.service, &fixture.context, &fixture.body) {
                Err(refusal) => refusal,
                Ok(_) => panic!("foreign State/Kura consumed original reservations"),
            };
        assert!(matches!(error, V2ApplyError::TaskMismatch));
        let original_provider = fixture.service.provider_ingest_finalized_archive.take();
        let (owner, error) =
            match owner.into_captures(&fixture.service, &fixture.context, &fixture.body) {
                Err(refusal) => refusal,
                Ok(_) => panic!("changed archive configuration consumed original reservations"),
            };
        assert!(matches!(error, V2ApplyError::TaskMismatch));
        fixture.service.provider_ingest_finalized_archive = original_provider;
        drop(busy_error(&fixture));
        let captures = owner
            .into_captures(&fixture.service, &fixture.context, &fixture.body)
            .unwrap();
        drop(captures);
        drop(reserve(&fixture).unwrap());
    }

    #[test]
    fn local_archive_failure_requires_recovery() {
        for provider_failure in [true, false] {
            let mut fixture = ApplyFixture::new_for_production_recovered_decision_apply();
            let archives = Archives::install(&mut fixture);
            let calls = capture_count(&fixture);
            let component = if provider_failure {
                "provider"
            } else {
                "reputation"
            };
            std::fs::rename(
                archives.directory.path().join(component),
                archives.directory.path().join("detached"),
            )
            .unwrap();
            let error = reserve(&fixture).unwrap_err();
            assert!(error.rejection_identity().is_none());
            assert!(error.requires_restart_recovery());
            assert!(matches!(
                error,
                V2ApplyError::LocalValidation(LocalValidationRefusal::RecoveryRequired(_))
            ));
            if !provider_failure {
                drop(archives.provider_owner(&fixture));
            }
            assert_eq!(capture_count(&fixture), calls);
        }
    }
}
