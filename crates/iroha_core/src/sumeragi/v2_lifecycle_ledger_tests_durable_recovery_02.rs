
        #[test]
        fn complete_tip_terminal_apply_store_join_rejects_store_drift() {
            let fixture = RecoveryFixture::new("complete-tip-predecessor-drift", 0x49);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let directory = TempDir::new().expect("temporary drifted predecessor ledger");
            let complete_tip =
                complete_tip_for_terminal_decision_at(&fixture, &projection, directory.path());
            let (store, empty) =
                LifecycleLedgerStoreV1::open(directory.path(), fixture.lifecycle_context())
                    .expect("open drifted CompleteTip predecessor store");
            store
                .persist(&ledger)
                .expect("persist terminal CompleteTip predecessor");
            store
                .persist(&empty)
                .expect("replace predecessor before cut authentication");

            assert!(
                ledger
                    .into_complete_tip_terminal_apply_store_join(store, complete_tip)
                    .is_err(),
                "a changed attached frame cannot mint predecessor authority"
            );
        }

        #[test]
        fn complete_tip_terminal_apply_store_join_rejects_an_identical_foreign_target() {
            let fixture = RecoveryFixture::new("complete-tip-predecessor-foreign-target", 0x4A);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let canonical_kura = Kura::blank_kura_for_testing();
            let foreign_kura = Kura::blank_kura_for_testing();
            let complete_tip = complete_tip_for_terminal_decision_on_kura(
                &fixture,
                &projection,
                canonical_kura.as_ref(),
            );
            let foreign_root = foreign_kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(fixture.verified.context().id().0.as_ref()));
            let (foreign_store, empty) =
                LifecycleLedgerStoreV1::open(&foreign_root, fixture.lifecycle_context())
                    .expect("open foreign predecessor store");
            assert!(empty.records().is_empty());
            foreign_store
                .persist(&ledger)
                .expect("copy exact terminal predecessor frame to foreign root");

            assert!(
                ledger
                    .into_complete_tip_terminal_apply_store_join(foreign_store, complete_tip)
                    .is_err(),
                "byte-identical ledger data cannot substitute for the Kura-bound target"
            );
        }

        #[test]
        fn complete_tip_successor_target_initializes_and_accepts_an_exact_descendant() {
            let context = LifecycleContext::new(LifecycleDigest::new([0xA1; 32]), 2);
            let directory = TempDir::new().expect("temporary CompleteTip successor target");
            let target = CanonicalCompleteTipSuccessorLedgerTargetV1 {
                root: directory.path().join("successor"),
                context,
            };
            let (store, initialized) = target
                .open_initialized_or_descendant(4)
                .expect("initialize successor at predecessor high-water");
            assert_eq!(initialized.high_water(), 4);
            assert!(initialized.records().is_empty());

            let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xA2; 32])), 5);
            let descendant = LifecycleLedgerV1::new(
                context,
                5,
                vec![unrelated_live_record(context, owner, 5, 0xA3)],
                BTreeMap::new(),
            )
            .expect("construct exact successor descendant");
            store
                .persist_exact_successor(&initialized, &descendant)
                .expect("publish descendant above retained ordinal floor");

            let (_, reopened) = target
                .open_initialized_or_descendant(4)
                .expect("preserve a valid nonempty descendant without rewriting it");
            assert_eq!(reopened, descendant);
        }

        #[test]
        fn complete_tip_successor_target_rejects_a_foreign_ordinal_floor() {
            let context = LifecycleContext::new(LifecycleDigest::new([0xB1; 32]), 2);
            let directory = TempDir::new().expect("temporary foreign-floor successor target");
            let target = CanonicalCompleteTipSuccessorLedgerTargetV1 {
                root: directory.path().join("successor"),
                context,
            };
            let (store, empty) = LifecycleLedgerStoreV1::open(&target.root, context)
                .expect("open foreign-floor successor");
            let owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xB2; 32])), 4);
            let foreign = LifecycleLedgerV1::new(
                context,
                4,
                vec![unrelated_live_record(context, owner, 4, 0xB3)],
                BTreeMap::new(),
            )
            .expect("construct independently zero-based successor frame");
            store
                .persist_exact_successor(&empty, &foreign)
                .expect("persist foreign successor fixture");

            assert!(target.open_initialized_or_descendant(4).is_err());
        }

        #[test]
        fn terminal_recovered_decision_oracle_rejects_a_live_apply() {
            let fixture = RecoveryFixture::new("terminal-decision-live-apply", 0x35);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let mut records = ledger.records.clone();
            records[3].terminal = None;
            let live = LifecycleLedgerV1::new(
                ledger.context(),
                ledger.high_water(),
                records,
                BTreeMap::new(),
            )
            .expect("construct otherwise exact chain with a live Apply");

            assert!(
                live.authenticate_terminal_recovered_decision_apply_projection(&projection)
                    .is_err()
            );
        }

        #[test]
        fn terminal_recovered_decision_oracle_rejects_extra_same_owner_history() {
            let fixture = RecoveryFixture::new("terminal-decision-same-owner", 0x39);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let owner = projection.fetch.owner();
            let mut records = ledger.records.clone();
            records.push(unrelated_live_record(ledger.context(), owner, 5, 0xE2));
            let with_extra_owner_history =
                LifecycleLedgerV1::new(ledger.context(), 5, records, BTreeMap::new())
                    .expect("construct terminal chain with foreign same-owner history");

            assert!(
                with_extra_owner_history
                    .authenticate_terminal_recovered_decision_apply_projection(&projection)
                    .is_err()
            );
        }

        #[test]
        fn terminal_recovered_decision_oracle_is_chain_local_and_allows_a_foreign_live_row() {
            let fixture = RecoveryFixture::new("terminal-decision-chain-local", 0x3D);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
            let foreign_root = CausalRoot::new(LifecycleDigest::new(
                *Hash::new(b"foreign live row outside terminal Decision chain").as_ref(),
            ));
            let foreign_owner = OwnerId::new(foreign_root, 5);
            let mut records = ledger.records.clone();
            records.push(unrelated_live_record(
                ledger.context(),
                foreign_owner,
                5,
                0xE3,
            ));
            let with_foreign_live =
                LifecycleLedgerV1::new(ledger.context(), 5, records, BTreeMap::new())
                    .expect("construct terminal chain beside one foreign live row");

            assert_eq!(
                with_foreign_live
                    .authenticate_terminal_recovered_decision_apply_projection(&projection)
                    .expect("the terminal oracle is intentionally limited to one owner chain"),
                4
            );
        }

        #[test]
        fn recovered_decision_stage_guard_routes_terminal_chain_to_complete_tip_retirement() {
            let fixture = RecoveryFixture::new("terminal-decision-stage-guard", 0x41);
            let (ledger, projection) = terminal_decision_chain_fixture(&fixture);

            let error = ledger
                .reject_terminal_recovered_decision_apply_projection(&projection)
                .expect_err("terminal Apply cannot re-enter live recovered staging");
            assert!(matches!(
                error,
                LifecycleLedgerError::InvalidLedger(reason)
                    if reason == "terminal recovered Decision Apply requires CompleteTip retirement, not a live carrier"
            ));
        }

        fn admit_and_claim_serve(
            fixture: &RecoveryFixture,
            owner: &mut ProductionLifecycleOwnerV1,
            request: &AuthenticatedCertifiedBodyRequest,
        ) -> super::super::super::TurnLease {
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );
            let admitted = owner.admit_selected_certified_serve(target, &fixture.keys[0], request);
            assert!(matches!(
                admitted.decision(),
                Some(super::super::super::AdmissionDecision::Admitted { .. })
            ));
            owner.claim_certified_serve_for_test()
        }

        #[test]
        fn consuming_storage_cut_censes_every_live_fetch_and_binds_exact_ledger_frame() {
            let fixture = RecoveryFixture::new("durable-ready-fetch-census", 0x31);
            let directory = TempDir::new().expect("temporary durable Ready-Fetch store");
            let mut store = fixture.open_store(&directory);
            let first = fixture.fetch_record(&mut store, 0, 0x41, 1, None, false);
            let second = fixture.fetch_record(&mut store, 1, 0x42, 2, None, false);
            let ledger = fixture.ledger(vec![first, second]);
            let ledger_directory =
                TempDir::new().expect("temporary durable Ready-Fetch lifecycle ledger");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);

            let mut cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    store,
                )
                .expect("all live durable Fetch rows form one consuming storage cut");
            assert_eq!(
                cut.ledger
                    .records
                    .iter()
                    .filter(|record| record.work_class() == Some(LifecycleWorkClass::Fetch))
                    .count(),
                2,
            );
            assert!(cut.is_exact(), "the opaque census covers both live rows");

            cut.ledger.high_water += 1;
            assert!(
                !cut.is_exact(),
                "the census cannot cross even a structurally harmless foreign ledger frame",
            );
        }

        #[test]
        fn production_owner_opens_empty_and_two_fetch_storage_atomically() {
            let empty_fixture = RecoveryFixture::new("empty-production-lifecycle-owner", 0x11);
            let empty_body_directory =
                TempDir::new().expect("temporary empty production body store");
            let empty_body_store = empty_fixture.open_store(&empty_body_directory);
            let empty_payload_directory =
                TempDir::new().expect("temporary empty production payload store");
            let (empty_payload_store, empty_payloads) = empty_fixture
                .open_empty_serve_payloads(&empty_payload_directory, &empty_body_store);
            let empty_ledger = empty_fixture.ledger(Vec::new());
            let empty_ledger_directory =
                TempDir::new().expect("temporary empty production ledger store");
            let empty_ledger_store =
                empty_fixture.persist_ledger(&empty_ledger_directory, &empty_ledger);
            let empty_cut = empty_ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    empty_fixture.verified.clone(),
                    empty_ledger_store,
                    empty_body_store,
                )
                .expect("seal empty production storage cut");
            let mut empty_owner = empty_cut
                .open_owner_for_test(empty_payload_store, empty_payloads)
                .expect("open empty production lifecycle owner");
            assert!(empty_owner.exact_recovered_fetch_join_for_test());
            assert_eq!(empty_owner.live_fetch_count_for_test(), 0);
            assert_eq!(empty_owner.plan_direct_registry_turn(), Ok(TurnPlan::Idle));

            let fixture = RecoveryFixture::new("two-fetch-production-lifecycle-owner", 0x21);
            let body_directory = TempDir::new().expect("temporary two-Fetch body store");
            let mut body_store = fixture.open_store(&body_directory);
            let first = fixture.fetch_record(&mut body_store, 0, 0x31, 1, None, false);
            let second = fixture.fetch_record(&mut body_store, 1, 0x32, 2, None, false);
            let payload_directory = TempDir::new().expect("temporary two-Fetch payload store");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger = fixture.ledger(vec![first, second]);
            let ledger_directory = TempDir::new().expect("temporary two-Fetch ledger store");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal two-Fetch production storage cut");
            let mut owner = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("open two-Fetch production lifecycle owner");
            assert!(owner.exact_recovered_fetch_join_for_test());
            assert_eq!(owner.live_fetch_count_for_test(), 2);
        }

        #[test]
        fn production_owner_keeps_terminal_validate_and_live_serve_together() {
            let fixture = RecoveryFixture::new("terminal-validate-live-serve-owner", 0x41);
            let body_directory = TempDir::new().expect("temporary coexistence body store");
            let mut body_store = fixture.open_store(&body_directory);
            let terminal_validate = fixture.terminal_validate_record(&mut body_store, 1, 0x51, 3);

            let payload_directory = TempDir::new().expect("temporary coexistence payload store");
            let (mut payload_store, _) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("open coexistence Certified-Serve payload store");
            let request = fixture.authenticated_serve_request(0, 0x52, 3);
            let receipt = payload_store
                .persist_pending_with_verified_retention(
                    &fixture.verified,
                    &fixture.keys[0],
                    &request,
                )
                .expect("persist coexistence Certified-Serve request");
            let authority =
                authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
                    .expect("construct coexistence lifecycle authority");
            let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
            assert!(matches!(
                coordinator
                    .admit_certified_serve(&fixture.verified, &request, receipt)
                    .expect("project coexistence Certified-Serve request"),
                super::super::super::AdmissionDecision::Admitted { .. }
            ));
            let serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
                .expect("project coexistence Serve ledger");
            let mut records = serve_ledger.records.clone();
            records.push(terminal_validate);
            let producer_debts = serve_ledger
                .producer_debts
                .iter()
                .map(|debt| (debt.serve_ordinal(), debt.producer_ordinal()))
                .collect();
            let ledger =
                LifecycleLedgerV1::new(fixture.lifecycle_context(), 3, records, producer_debts)
                    .expect("construct terminal-Validate/live-Serve ledger");
            drop(payload_store);
            let (payload_store, recovered_payloads) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("reopen coexistence Certified-Serve payload store");
            let payloads = recovered_payloads
                .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
                .expect("authenticate coexistence Certified-Serve payload");
            let ledger_directory = TempDir::new().expect("temporary coexistence ledger store");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal coexistence storage cut");
            let mut owner = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("open terminal-Validate/live-Serve production owner");
            assert!(owner.exact_recovered_fetch_join_for_test());
            assert_eq!(owner.live_fetch_count_for_test(), 0);
            assert_eq!(owner.terminal_validate_count_for_test(), 1);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1),
                "live Serve and dormant adjacent ProducerTurn both retain exact carriers",
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .one_certified_serve_pair_shares_replay_family(),
                "startup carriers retain the same whole replay family",
            );
        }

        #[test]
        fn fresh_certified_serve_publishes_exact_ledger_and_shared_pair_beside_fetch() {
            let fixture = RecoveryFixture::new("fresh-serve-owner", 0x81);
            let body_directory = TempDir::new().expect("temporary fresh Serve body store");
            let mut body_store = fixture.open_store(&body_directory);
            let fetch = fixture.fetch_record(&mut body_store, 0, 0x82, 1, None, false);
            let payload_directory = TempDir::new().expect("temporary fresh Serve payload store");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger = fixture.ledger(vec![fetch]);
            let ledger_directory = TempDir::new().expect("temporary fresh Serve ledger store");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal fresh Serve storage cut");
            let mut owner = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("open fresh Serve production owner");
            let request = fixture.authenticated_serve_request(1, 0x83, 3);
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );

            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(matches!(
                outcome.decision(),
                Some(super::super::super::AdmissionDecision::Admitted {
                    ordinal: 2,
                    producer_turn_ordinal: Some(3),
                    ..
                })
            ));
            assert!(!outcome.restart_required());
            let Ok(continuation) = outcome.into_safe_continuation() else {
                panic!("published fresh Serve must return its safe selector continuation")
            };
            assert!(continuation.failure().is_none());
            assert!(
                continuation
                    .into_target()
                    .matches_certified_serve_request(request.request_hash())
            );
            assert_eq!(owner.live_fetch_count_for_test(), 1);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .one_certified_serve_pair_shares_replay_family()
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .exactly_covers_recovered_ready_work(&owner.coordinator)
            );
            let store = owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("fresh owner retains LedgerV1 store");
            assert_eq!(
                store.load().expect("reload fresh Serve LedgerV1"),
                LifecycleLedgerV1::from_coordinator(&owner.coordinator)
                    .expect("project fresh Serve coordinator")
            );

            let retry_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    2,
                );
            let retry =
                owner.admit_selected_certified_serve(retry_target, &fixture.keys[0], &request);
            assert!(matches!(
                retry.decision(),
                Some(super::super::super::AdmissionDecision::Retry { ordinal: 2, .. })
            ));
            assert!(retry.into_safe_continuation().is_ok());
            assert_eq!(owner.live_fetch_count_for_test(), 1);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1),
                "idempotent retry must preserve the unrelated Fetch and exact shared pair"
            );
        }

        #[test]
        fn terminal_owner_publishes_completed_and_reopens_exact_producer_carrier() {
            let fixture = RecoveryFixture::new("terminal-owner-completed", 0x85);
            let body_directory = TempDir::new().expect("temporary completed-owner body store");
            let payload_directory =
                TempDir::new().expect("temporary completed-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary completed-owner ledger store");
            let (mut owner, request, durable_body, response) = fixture.open_completed_serve_owner(
                &body_directory,
                &payload_directory,
                &ledger_directory,
            );
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let serve_ordinal = lease.ordinal();
            let producer_ordinal = serve_ordinal + 1;

            owner
                .settle_certified_serve_completed(lease, &request, &durable_body, &response)
                .expect("owner publishes exact completed Serve terminal");

            let response_digest =
                LifecycleDigest::new((*iroha_crypto::HashOf::new(&response).as_ref()).into());
            assert_eq!(
                owner.coordinator.records[&serve_ordinal].state,
                LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
            );
            assert_eq!(
                owner.coordinator.records[&producer_ordinal].state,
                LifecycleState::Ready
            );
            assert_eq!(owner.coordinator.active_lease, None);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .exactly_covers_recovered_ready_work(&owner.coordinator)
            );
            let on_disk = owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("completed owner retains LedgerV1 store")
                .load()
                .expect("reload completed owner LedgerV1");
            assert_eq!(
                on_disk,
                LifecycleLedgerV1::from_coordinator(&owner.coordinator)
                    .expect("project completed owner coordinator")
            );
            drop(owner);

            let body_store = fixture.open_store(&body_directory);
            let (payload_store, recovered) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("reopen completed-owner payload store");
            let payloads = recovered
                .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
                .expect("authenticate completed-owner payloads");
            let (ledger_store, ledger) =
                LifecycleLedgerStoreV1::open(ledger_directory.path(), fixture.lifecycle_context())
                    .expect("reopen completed-owner LedgerV1");
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal completed-owner restart cut");
            let mut reopened = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("reopen completed production owner");
            assert_eq!(
                reopened.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );
            assert_eq!(
                reopened.coordinator.records[&serve_ordinal].state,
                LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
            );
            assert!(
                reopened
                    .registry
                    .registry_mut()
                    .exactly_covers_recovered_ready_work(&reopened.coordinator)
            );
        }

        #[test]
        fn terminal_owner_publishes_rejected_failed_and_cancelled_carrier_shapes() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            for (index, outcome) in [
                CertifiedServePayloadNegativeOutcome::Rejected(37),
                CertifiedServePayloadNegativeOutcome::Failed(41),
                CertifiedServePayloadNegativeOutcome::Cancelled,
            ]
            .into_iter()
            .enumerate()
            {
                let fixture = RecoveryFixture::new(
                    &format!("terminal-owner-negative-{index}"),
                    0x89 + u8::try_from(index).expect("small terminal fixture index") * 4,
                );
                let body_directory = TempDir::new().expect("temporary negative-owner body store");
                let payload_directory =
                    TempDir::new().expect("temporary negative-owner payload store");
                let ledger_directory =
                    TempDir::new().expect("temporary negative-owner ledger store");
                let mut owner = fixture.open_empty_owner(
                    &body_directory,
                    &payload_directory,
                    &ledger_directory,
                );
                let request = fixture.authenticated_serve_request(0, 0x90, 3);
                let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
                let serve_ordinal = lease.ordinal();
                let producer_ordinal = serve_ordinal + 1;

                owner
                    .settle_certified_serve_negative(lease, &request, outcome)
                    .expect("owner publishes exact negative Serve terminal");

                let expected = match outcome {
                    CertifiedServePayloadNegativeOutcome::Rejected(code) => {
                        TerminalOutcome::Rejected(code)
                    }
                    CertifiedServePayloadNegativeOutcome::Failed(code) => {
                        TerminalOutcome::Failed(code)
                    }
                    CertifiedServePayloadNegativeOutcome::Cancelled => TerminalOutcome::Cancelled,
                };
                assert_eq!(
                    owner.coordinator.records[&serve_ordinal].state,
                    LifecycleState::Terminal(expected)
                );
                let cancelled = outcome == CertifiedServePayloadNegativeOutcome::Cancelled;
                assert_eq!(
                    owner.coordinator.records[&producer_ordinal].state,
                    if cancelled {
                        LifecycleState::Terminal(TerminalOutcome::Cancelled)
                    } else {
                        LifecycleState::Ready
                    }
                );
                assert_eq!(
                    owner.certified_serve_and_producer_carrier_counts_for_test(),
                    if cancelled { (0, 0) } else { (0, 1) }
                );
                assert_eq!(
                    owner.coordinator.producer_debts.get(&serve_ordinal),
                    (!cancelled).then_some(&producer_ordinal)
                );
                assert!(
                    owner
                        .registry
                        .registry_mut()
                        .exactly_covers_recovered_ready_work(&owner.coordinator)
                );
            }
        }

        #[test]
        fn terminal_owner_returns_foreign_request_and_body_before_publication() {
            let fixture = RecoveryFixture::new("terminal-owner-input-rejection", 0x99);
            let body_directory = TempDir::new().expect("temporary input-owner body store");
            let payload_directory = TempDir::new().expect("temporary input-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary input-owner ledger store");
            let (mut owner, request, durable_body, response) = fixture.open_completed_serve_owner(
                &body_directory,
                &payload_directory,
                &ledger_directory,
            );
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            let payloads = snapshot_files(payload_directory.path());
            let foreign = fixture.authenticated_serve_request(1, 0x9A, 3);

            let mut foreign_lease = lease.clone();
            foreign_lease.ordinal = foreign_lease
                .ordinal
                .checked_add(2)
                .expect("small foreign lease ordinal");
            let error = owner
                .settle_certified_serve_completed(foreign_lease, &request, &durable_body, &response)
                .expect_err("foreign lease is rejected before terminal persistence");
            assert!(!error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::Coordinator
            );
            assert!(error.into_lease().is_ok());
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);

            let error = owner
                .settle_certified_serve_completed(lease, &foreign, &durable_body, &response)
                .expect_err("foreign request is rejected before terminal persistence");
            assert!(!error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::RequestAuthority
            );
            let lease = error
                .into_lease()
                .expect("prepublication rejection returns the exact active lease");
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );

            let foreign_receipt = crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
                fixture.verified.context().id(),
                response.manifest.round,
                response.manifest.subject,
                iroha_crypto::HashOf::new(&response.manifest),
            );
            let error = owner
                .settle_certified_serve_completed(lease, &request, &foreign_receipt, &response)
                .expect_err("foreign durable receipt is rejected before terminal persistence");
            assert!(!error.restart_required());
            let lease = error
                .into_lease()
                .expect("foreign body receipt returns the exact active lease");
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );

            let mut foreign_body = response.clone();
            foreign_body.body.push(0);
            let error = owner
                .settle_certified_serve_completed(lease, &request, &durable_body, &foreign_body)
                .expect_err("foreign response body is rejected before terminal persistence");
            assert!(!error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
            );
            let lease = error
                .into_lease()
                .expect("foreign response body returns the exact active lease");
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);

            let retained_body_store = owner
                .body_store
                .take()
                .expect("unlaunched owner still retains its exact body store");
            let error = owner
                .settle_certified_serve_completed(lease, &request, &durable_body, &response)
                .expect_err("completion without the retained body store is prepublication-safe");
            assert!(!error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::BodyStoreUnavailable
            );
            let lease = error
                .into_lease()
                .expect("unavailable body store returns the exact active lease");
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(lease));
            assert_eq!(owner.coordinator.fault(), None);
            assert_eq!(snapshot_files(payload_directory.path()), payloads);
            drop(retained_body_store);
        }

        #[test]
        fn terminal_owner_faults_on_corrupt_owned_body_after_receipt_mint() {
            let fixture = RecoveryFixture::new("terminal-owner-owned-body-corruption", 0x9B);
            let body_directory = TempDir::new().expect("temporary corrupt-owner body store");
            let payload_directory = TempDir::new().expect("temporary corrupt-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary corrupt-owner ledger store");
            let (mut owner, request, durable_body, response) = fixture.open_completed_serve_owner(
                &body_directory,
                &payload_directory,
                &ledger_directory,
            );
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let active_lease = lease.clone();
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            let pending_payloads = snapshot_files(payload_directory.path());
            let ledger = owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("terminal owner retains LedgerV1 store")
                .load()
                .expect("load pre-corruption LedgerV1");
            owner
                .body_store
                .as_ref()
                .expect("unlaunched owner retains its exact body store")
                .corrupt_owned_frame_for_test(&durable_body)
                .expect("replace the already-accepted body frame");

            let error = owner
                .settle_certified_serve_completed(lease, &request, &durable_body, &response)
                .expect_err("reload corruption after receipt ownership requires restart");
            assert!(error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
            );
            assert!(
                error.into_lease().is_err(),
                "accepted-store corruption must not release a safe retry lease"
            );
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(active_lease));
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert_eq!(snapshot_files(payload_directory.path()), pending_payloads);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .one_certified_serve_pair_shares_replay_family()
            );
            assert_eq!(
                owner
                    .coordinator
                    .ledger_store
                    .as_ref()
                    .expect("faulted owner retains LedgerV1 store")
                    .load()
                    .expect("reload unchanged LedgerV1"),
                ledger
            );
        }

        #[test]
        fn terminal_registry_rejects_every_arbitrary_staged_drift_before_callback() {
            for (index, drift) in [
                StagedTerminalDrift::Record,
                StagedTerminalDrift::Index,
                StagedTerminalDrift::Debt,
                StagedTerminalDrift::Capacity,
                StagedTerminalDrift::HighWater,
            ]
            .into_iter()
            .enumerate()
            {
                let fixture = RecoveryFixture::new(
                    &format!("terminal-staged-drift-{index}"),
                    0xB0 + u8::try_from(index).expect("small drift index") * 4,
                );
                let body_directory = TempDir::new().expect("temporary staged-drift body store");
                let payload_directory =
                    TempDir::new().expect("temporary staged-drift payload store");
                let ledger_directory = TempDir::new().expect("temporary staged-drift ledger store");
                let (mut owner, request, durable_body, response) = fixture
                    .open_completed_serve_owner(
                        &body_directory,
                        &payload_directory,
                        &ledger_directory,
                    );
                let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
                let serve_ordinal = lease.ordinal();
                let producer_ordinal = owner.coordinator.producer_debts[&serve_ordinal];
                let receipt = owner
                    .payload_store
                    .persist_completed_with_exact_body(
                        &request,
                        &durable_body,
                        owner
                            .body_store
                            .as_ref()
                            .expect("unlaunched owner retains body store"),
                        &response,
                    )
                    .expect("persist terminal receipt for staged-drift preflight");
                let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
                    owner.coordinator.active_context,
                    &owner.coordinator.records[&serve_ordinal],
                    &owner.coordinator.durable_records[&serve_ordinal],
                    &owner.coordinator.records[&producer_ordinal],
                    &owner.coordinator.durable_records[&producer_ordinal],
                    receipt,
                )
                .expect("seal exact terminal replay pair");
                let transition = owner
                    .registry
                    .registry_mut()
                    .prepare_certified_serve_terminal_transition(
                        &owner.coordinator,
                        &lease,
                        &request,
                        &terminal,
                    )
                    .expect("prepare exact terminal registry transition");
                let outcome = terminal.terminal_outcome();
                let mut staged = owner.coordinator.stage_durable_transaction();
                staged.reduce_settle_turn(
                    lease.clone(),
                    super::super::super::TurnOutcome::Terminal(outcome),
                    Some(terminal),
                );
                assert_eq!(staged.fault(), None);

                match drift {
                    StagedTerminalDrift::Record => {
                        let mut extra = staged.records[&producer_ordinal].clone();
                        extra.ordinal = u128::MAX - 1;
                        assert!(staged.records.insert(extra.ordinal, extra).is_none());
                    }
                    StagedTerminalDrift::Index => {
                        let key = staged.records[&serve_ordinal].key;
                        assert!(staged.key_index.remove(&key).is_some());
                    }
                    StagedTerminalDrift::Debt => {
                        assert!(staged.producer_debts.remove(&serve_ordinal).is_some());
                    }
                    StagedTerminalDrift::Capacity => {
                        *staged
                            .capacity_used
                            .get_mut(&super::super::super::CapacityClass::Effect)
                            .expect("effect capacity counter exists") += 1;
                    }
                    StagedTerminalDrift::HighWater => {
                        staged.high_water = staged
                            .high_water
                            .checked_add(1)
                            .expect("fixture high-water has room");
                    }
                }

                let records = owner.coordinator.records.clone();
                let durable_records = owner.coordinator.durable_records.clone();
                let mut callback_invoked = false;
                let result = owner
                    .registry
                    .registry_mut()
                    .publish_certified_serve_terminal_transition(
                        transition,
                        &owner.coordinator,
                        &staged,
                        &lease,
                        || {
                            callback_invoked = true;
                            Ok::<(), ()>(())
                        },
                    );
                assert!(matches!(
                    result,
                    Err(
                        super::super::super::work_registry::CertifiedServeTerminalRegistryPublicationError::Preflight(
                            _
                        )
                    )
                ));
                assert!(!callback_invoked);
                assert_eq!(owner.coordinator.records, records);
                assert_eq!(owner.coordinator.durable_records, durable_records);
                assert_eq!(owner.coordinator.active_lease, Some(lease.clone()));
                assert_eq!(
                    owner.certified_serve_and_producer_carrier_counts_for_test(),
                    (1, 1)
                );
                assert!(
                    owner
                        .registry
                        .registry_mut()
                        .one_certified_serve_pair_shares_replay_family()
                );
                assert!(
                    owner
                        .registry
                        .registry_mut()
                        .preflight_certified_serve_terminal_owner_state(
                            &owner.coordinator,
                            &lease,
                        )
                );
            }
        }

        #[test]
        fn terminal_owner_registry_mismatch_faults_before_payload_persistence() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            let fixture = RecoveryFixture::new("terminal-owner-registry-mismatch", 0x9D);
            let body_directory = TempDir::new().expect("temporary registry-owner body store");
            let payload_directory = TempDir::new().expect("temporary registry-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary registry-owner ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0x9E, 3);
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let active_lease = lease.clone();
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .remove_one_certified_serve_carrier_for_test()
            );
            let payloads = snapshot_files(payload_directory.path());

            let error = owner
                .settle_certified_serve_negative(
                    lease,
                    &request,
                    CertifiedServePayloadNegativeOutcome::Rejected(43),
                )
                .expect_err("private registry mismatch requires restart");
            assert!(error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::Registry
            );
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(active_lease));
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert_eq!(snapshot_files(payload_directory.path()), payloads);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1),
                "terminal preflight must not mutate the already-mismatched registry"
            );
        }

        #[test]
        fn terminal_owner_ledger_drift_restores_both_current_carriers() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            let fixture = RecoveryFixture::new("terminal-owner-ledger-drift", 0xA1);
            let body_directory = TempDir::new().expect("temporary drift-owner body store");
            let payload_directory = TempDir::new().expect("temporary drift-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary drift-owner ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0xA2, 3);
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let active_lease = lease.clone();
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("terminal owner retains LedgerV1 store")
                .persist(&fixture.ledger(Vec::new()))
                .expect("drift the on-disk LedgerV1 before terminal publication");
            let pending_payloads = snapshot_files(payload_directory.path());

            let error = owner
                .settle_certified_serve_negative(
                    lease,
                    &request,
                    CertifiedServePayloadNegativeOutcome::Failed(47),
                )
                .expect_err("exact LedgerV1 drift rejects terminal successor");
            assert!(error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::Ledger
            );
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(active_lease));
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1),
                "Ledger failure restores the byte-for-byte current Serve/Producer pair"
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .one_certified_serve_pair_shares_replay_family()
            );
            assert_ne!(
                snapshot_files(payload_directory.path()),
                pending_payloads,
                "the fsynced terminal payload remains as a startup reconciliation tail"
            );
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
        }

        #[test]
        fn terminal_owner_postrename_sync_failure_keeps_logical_and_registry_state() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            let fixture = RecoveryFixture::new("terminal-owner-postrename", 0xA5);
            let body_directory = TempDir::new().expect("temporary postrename-owner body store");
            let payload_directory =
                TempDir::new().expect("temporary postrename-owner payload store");
            let ledger_directory = TempDir::new().expect("temporary postrename-owner ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0xA6, 3);
            let lease = admit_and_claim_serve(&fixture, &mut owner, &request);
            let active_lease = lease.clone();
            let records = owner.coordinator.records.clone();
            let durable_records = owner.coordinator.durable_records.clone();
            let pending_payloads = snapshot_files(payload_directory.path());
            owner
                .payload_store
                .fail_next_publish_directory_sync_for_test();

            let error = owner
                .settle_certified_serve_negative(
                    lease,
                    &request,
                    CertifiedServePayloadNegativeOutcome::Rejected(53),
                )
                .expect_err("post-rename sync ambiguity requires restart");
            assert!(error.restart_required());
            assert_eq!(
                error.failure(),
                super::super::super::CertifiedServeTerminalSettlementFailureV1::PayloadStore
            );
            assert_eq!(owner.coordinator.records, records);
            assert_eq!(owner.coordinator.durable_records, durable_records);
            assert_eq!(owner.coordinator.active_lease, Some(active_lease));
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert_ne!(
                snapshot_files(payload_directory.path()),
                pending_payloads,
                "ambiguous renamed terminal frame remains for startup"
            );
        }

        #[test]
        fn fresh_certified_serve_rejects_foreign_target_and_rolls_back_capacity_wait() {
            let fixture = RecoveryFixture::new("fresh-serve-preledger", 0x91);
            let body_directory = TempDir::new().expect("temporary preledger body store");
            let payload_directory = TempDir::new().expect("temporary preledger payload store");
            let ledger_directory = TempDir::new().expect("temporary preledger ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0x92, 3);
            let foreign = fixture.authenticated_serve_request(1, 0x93, 3);
            let foreign_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    foreign.request_hash(),
                    1,
                );
            let payload_before = snapshot_files(payload_directory.path());
            let foreign_outcome =
                owner.admit_selected_certified_serve(foreign_target, &fixture.keys[0], &request);
            assert_eq!(
                foreign_outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::SelectorAuthority
                )
            );
            let Ok(foreign_continuation) = foreign_outcome.into_safe_continuation() else {
                panic!("foreign target rejection is a safe pre-persistence continuation")
            };
            let recovered_foreign_target = foreign_continuation.into_target();
            assert!(
                recovered_foreign_target.matches_certified_serve_request(foreign.request_hash())
            );
            assert!(
                !recovered_foreign_target.matches_certified_serve_request(request.request_hash())
            );
            assert_eq!(snapshot_files(payload_directory.path()), payload_before);

            let admitted_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    2,
                );
            assert!(
                owner
                    .admit_selected_certified_serve(admitted_target, &fixture.keys[0], &request)
                    .into_safe_continuation()
                    .is_ok()
            );
            let payload_after_first = snapshot_files(payload_directory.path());
            let waiting = fixture.authenticated_serve_request(2, 0x94, 3);
            let waiting_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    waiting.request_hash(),
                    3,
                );
            let waiting_outcome =
                owner.admit_selected_certified_serve(waiting_target, &fixture.keys[0], &waiting);
            assert!(matches!(
                waiting_outcome.decision(),
                Some(super::super::super::AdmissionDecision::WaitForCapacity(_))
            ));
            assert_eq!(
                waiting_outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                )
            );
            let Ok(waiting_continuation) = waiting_outcome.into_safe_continuation() else {
                panic!("proven Pending rollback must release the selector continuation")
            };
            assert!(
                waiting_continuation
                    .into_target()
                    .matches_certified_serve_request(waiting.request_hash())
            );
            assert_eq!(
                snapshot_files(payload_directory.path()),
                payload_after_first,
                "a proven pre-ledger capacity decline must synchronously remove only its fresh Pending frame"
            );
            assert_eq!(owner.coordinator.records.len(), 2);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (1, 1)
            );
        }

        #[test]
        fn fresh_certified_serve_postledger_failure_retains_tail_and_requires_restart() {
            let fixture = RecoveryFixture::new("fresh-serve-restart", 0xA1);
            let body_directory = TempDir::new().expect("temporary restart body store");
            let payload_directory = TempDir::new().expect("temporary restart payload store");
            let ledger_directory = TempDir::new().expect("temporary restart ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let changed =
                LifecycleLedgerV1::new(fixture.lifecycle_context(), 1, Vec::new(), BTreeMap::new())
                    .expect("construct changed pre-publication LedgerV1");
            owner
                .coordinator
                .ledger_store
                .as_ref()
                .expect("fresh owner retains LedgerV1 store")
                .persist(&changed)
                .expect("replace LedgerV1 before exact successor publication");
            let request = fixture.authenticated_serve_request(0, 0xA2, 3);
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );
            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(outcome.restart_required());
            assert_eq!(
                outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Ledger
                )
            );
            let Err(retained) = outcome.into_safe_continuation() else {
                panic!("post-ledger failure must not release the selector target")
            };
            assert!(retained.restart_required());
            drop(retained);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 0),
                "failed LedgerV1 publication rolls back both staged registry carriers"
            );
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert_ne!(
                snapshot_files(payload_directory.path()),
                BTreeMap::new(),
                "the authenticated post-fsync payload tail remains for restart recovery"
            );

            let reentry = fixture.authenticated_serve_request(1, 0xA3, 3);
            let reentry_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    reentry.request_hash(),
                    2,
                );
            let payload_before_reentry = snapshot_files(payload_directory.path());
            let reentry_outcome =
                owner.admit_selected_certified_serve(reentry_target, &fixture.keys[0], &reentry);
            assert!(reentry_outcome.restart_required());
            assert_eq!(
                reentry_outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                )
            );
            assert!(reentry_outcome.into_safe_continuation().is_err());
            assert_eq!(
                snapshot_files(payload_directory.path()),
                payload_before_reentry,
                "a faulted owner must retain the new selector target without touching payload storage"
            );
        }

        #[test]
        fn fresh_certified_serve_postrename_sync_failure_requires_restart() {
            let fixture = RecoveryFixture::new("fresh-serve-postrename-sync", 0xA5);
            let body_directory = TempDir::new().expect("temporary post-rename body store");
            let payload_directory = TempDir::new().expect("temporary post-rename payload store");
            let ledger_directory = TempDir::new().expect("temporary post-rename ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            owner
                .payload_store
                .fail_next_publish_directory_sync_for_test();
            let request = fixture.authenticated_serve_request(0, 0xA6, 3);
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );

            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(outcome.restart_required());
            assert_eq!(
                outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::PayloadStore
                )
            );
            assert!(outcome.into_safe_continuation().is_err());
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
            assert!(owner.coordinator.records.is_empty());
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 0)
            );
            assert_ne!(
                snapshot_files(payload_directory.path()),
                BTreeMap::new(),
                "the renamed frame is an opaque crash tail, never a retryable unchanged attempt"
            );
        }

        #[test]
        fn ledgerless_owner_requires_restart_before_selector_validation() {
            let fixture = RecoveryFixture::new("ledgerless-serve-owner", 0xA9);
            let body_directory = TempDir::new().expect("temporary ledgerless body store");
            let payload_directory = TempDir::new().expect("temporary ledgerless payload store");
            let ledger_directory = TempDir::new().expect("temporary ledgerless ledger store");
            let mut owner =
                fixture.open_empty_owner(&body_directory, &payload_directory, &ledger_directory);
            let request = fixture.authenticated_serve_request(0, 0xAA, 3);
            let foreign = fixture.authenticated_serve_request(1, 0xAB, 3);
            let foreign_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    foreign.request_hash(),
                    1,
                );
            let _detached_store = owner
                .coordinator
                .ledger_store
                .take()
                .expect("fresh owner starts with its exact LedgerV1 store");

            let outcome =
                owner.admit_selected_certified_serve(foreign_target, &fixture.keys[0], &request);
            assert!(outcome.restart_required());
            assert_eq!(
                outcome.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                )
            );
            assert!(outcome.into_safe_continuation().is_err());
            assert_eq!(snapshot_files(payload_directory.path()), BTreeMap::new());
        }

        #[test]
        fn completed_certified_serve_tombstone_replays_without_a_serve_carrier() {
            let fixture = RecoveryFixture::new("completed-serve-replay", 0xB1);
            let body_directory = TempDir::new().expect("temporary completed body store");
            let payload_directory = TempDir::new().expect("temporary completed payload store");
            let ledger_directory = TempDir::new().expect("temporary completed ledger store");
            let (mut owner, request) = fixture.open_terminal_serve_owner(
                &body_directory,
                &payload_directory,
                &ledger_directory,
                ServeTerminalFixture::Completed,
            );
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );
            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(matches!(
                outcome.decision(),
                Some(super::super::super::AdmissionDecision::ReplayTerminal {
                    outcome: TerminalOutcome::Completed(Some(_)),
                    ..
                })
            ));
            assert!(outcome.into_safe_continuation().is_ok());
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );

            let foreign_retainer_target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    2,
                );
            let foreign_retainer = owner.admit_selected_certified_serve(
                foreign_retainer_target,
                &fixture.keys[1],
                &request,
            );
            assert!(foreign_retainer.restart_required());
            assert_eq!(
                foreign_retainer.failure(),
                Some(
                    super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                )
            );
            assert!(foreign_retainer.into_safe_continuation().is_err());
            assert_eq!(
                owner.coordinator.fault(),
                Some(super::super::super::CoordinatorFault::DurabilityFailure)
            );
        }

        #[test]
        fn payload_store_ahead_terminal_startup_installs_only_the_live_producer() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            let fixture = RecoveryFixture::new("store-ahead-serve-replay", 0xB5);
            let body_directory = TempDir::new().expect("temporary store-ahead body store");
            let payload_directory = TempDir::new().expect("temporary store-ahead payload store");
            let ledger_directory = TempDir::new().expect("temporary store-ahead ledger store");
            let body_store = fixture.open_store(&body_directory);
            let request = fixture.authenticated_serve_request(0, 0xD3, 3);
            let (mut payload_store, recovery) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("open store-ahead Serve payload store");
            assert!(recovery.is_empty());
            let pending = payload_store
                .persist_pending_with_verified_retention(
                    &fixture.verified,
                    &fixture.keys[0],
                    &request,
                )
                .expect("persist store-ahead Pending frame");
            let authority =
                authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
                    .expect("construct store-ahead lifecycle authority");
            let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
            assert!(matches!(
                coordinator
                    .admit_certified_serve(&fixture.verified, &request, pending)
                    .expect("project store-ahead Serve request"),
                super::super::super::AdmissionDecision::Admitted { .. }
            ));
            let ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
                .expect("project Pending store-ahead LedgerV1");
            payload_store
                .persist_negative(
                    pending.id(),
                    CertifiedServePayloadNegativeOutcome::Rejected(29),
                )
                .expect("persist store-ahead negative tombstone");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            drop(payload_store);
            let (payload_store, recovered) = CertifiedServePayloadStoreV1::open(
                payload_directory.path(),
                fixture.verified.context(),
            )
            .expect("reopen store-ahead Serve payload store");
            let payloads = recovered
                .authenticate(&fixture.verified, &fixture.keys[0], &body_store)
                .expect("authenticate store-ahead Serve payload");
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal store-ahead storage cut");
            let mut owner = cut
                .open_owner_for_test(payload_store, payloads)
                .expect("open store-ahead production owner");

            assert_eq!(
                owner.coordinator.records[&1].state,
                LifecycleState::Terminal(TerminalOutcome::Rejected(29))
            );
            assert!(
                !owner.coordinator.records[&1].physical_slots.is_empty(),
                "store-ahead settlement retains non-executable former Pending geometry"
            );
            assert_eq!(owner.coordinator.records[&2].state, LifecycleState::Ready);
            assert_eq!(
                owner.certified_serve_and_producer_carrier_counts_for_test(),
                (0, 1)
            );
            assert!(
                owner
                    .registry
                    .registry_mut()
                    .exactly_covers_recovered_ready_work(&owner.coordinator)
            );
            let target =
                super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                    fixture.verified.context(),
                    request.request_hash(),
                    1,
                );
            let outcome = owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
            assert!(matches!(
                outcome.decision(),
                Some(super::super::super::AdmissionDecision::StutterTerminal { .. })
            ));
            assert!(outcome.into_safe_continuation().is_ok());
        }

        #[test]
        fn negative_and_cancelled_certified_serve_tombstones_stutter_exactly() {
            use crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome;

            for (index, terminal) in [
                CertifiedServePayloadNegativeOutcome::Rejected(17),
                CertifiedServePayloadNegativeOutcome::Cancelled,
            ]
            .into_iter()
            .enumerate()
            {
                let fixture = RecoveryFixture::new(
                    &format!("negative-serve-replay-{index}"),
                    0xC1 + u8::try_from(index).expect("small fixture index") * 4,
                );
                let body_directory = TempDir::new().expect("temporary negative body store");
                let payload_directory = TempDir::new().expect("temporary negative payload store");
                let ledger_directory = TempDir::new().expect("temporary negative ledger store");
                let (mut owner, request) = fixture.open_terminal_serve_owner(
                    &body_directory,
                    &payload_directory,
                    &ledger_directory,
                    ServeTerminalFixture::Negative(terminal),
                );
                let expected_carriers = match terminal {
                    CertifiedServePayloadNegativeOutcome::Cancelled => (0, 0),
                    CertifiedServePayloadNegativeOutcome::Rejected(_)
                    | CertifiedServePayloadNegativeOutcome::Failed(_) => (0, 1),
                };
                assert_eq!(
                    owner.certified_serve_and_producer_carrier_counts_for_test(),
                    expected_carriers
                );
                let target =
                    super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                        fixture.verified.context(),
                        request.request_hash(),
                        1,
                    );
                let outcome =
                    owner.admit_selected_certified_serve(target, &fixture.keys[0], &request);
                assert!(matches!(
                    outcome.decision(),
                    Some(super::super::super::AdmissionDecision::StutterTerminal { .. })
                ));
                assert!(outcome.into_safe_continuation().is_ok());
                assert_eq!(
                    owner.certified_serve_and_producer_carrier_counts_for_test(),
                    expected_carriers
                );

                let foreign_retainer_target =
                    super::super::super::LifecycleIngressIoTargetSeal::for_certified_serve_test(
                        fixture.verified.context(),
                        request.request_hash(),
                        2,
                    );
                let foreign_retainer = owner.admit_selected_certified_serve(
                    foreign_retainer_target,
                    &fixture.keys[1],
                    &request,
                );
                assert!(foreign_retainer.restart_required());
                assert_eq!(
                    foreign_retainer.failure(),
                    Some(
                        super::super::super::projection::CertifiedServeConcreteAdmissionFailureV1::Coordinator
                    )
                );
                assert!(foreign_retainer.into_safe_continuation().is_err());
                assert_eq!(
                    owner.coordinator.fault(),
                    Some(super::super::super::CoordinatorFault::DurabilityFailure)
                );
            }
        }

        #[test]
        fn production_owner_rejects_changed_store_and_corrupt_census_without_further_writes() {
            let fixture = RecoveryFixture::new("changed-production-owner-store", 0x61);
            let body_directory = TempDir::new().expect("temporary changed-store body root");
            let body_store = fixture.open_store(&body_directory);
            let payload_directory = TempDir::new().expect("temporary changed-store payload root");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger = fixture.ledger(Vec::new());
            let ledger_directory = TempDir::new().expect("temporary changed-store ledger root");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal changed-store production cut");
            let changed =
                LifecycleLedgerV1::new(fixture.lifecycle_context(), 1, Vec::new(), BTreeMap::new())
                    .expect("construct same-context changed ledger frame");
            cut.ledger_store
                .persist(&changed)
                .expect("replace the retained store after cut mint");
            let ledger_after_external_change = snapshot_files(ledger_directory.path());
            let body_before_failure = snapshot_files(body_directory.path());
            let payload_before_failure = snapshot_files(payload_directory.path());
            let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
                panic!("same-store frame change must fail closed")
            };
            assert!(matches!(
                error.kind,
                ProductionLifecycleStartupErrorKindV1::InvalidStorageCut
                    | ProductionLifecycleStartupErrorKindV1::LedgerFrameMismatch
            ));
            assert_eq!(
                snapshot_files(ledger_directory.path()),
                ledger_after_external_change
            );
            assert_eq!(snapshot_files(body_directory.path()), body_before_failure);
            assert_eq!(
                snapshot_files(payload_directory.path()),
                payload_before_failure
            );

            let fixture = RecoveryFixture::new("corrupt-production-owner-census", 0x71);
            let body_directory = TempDir::new().expect("temporary corrupt-census body root");
            let mut body_store = fixture.open_store(&body_directory);
            let fetch = fixture.fetch_record(&mut body_store, 0, 0x72, 1, None, false);
            let payload_directory = TempDir::new().expect("temporary corrupt-census payload root");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger = fixture.ledger(vec![fetch]);
            let ledger_directory = TempDir::new().expect("temporary corrupt-census ledger root");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let mut cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal corrupt-census production cut");
            cut.corrupt_fetch_census_for_test();
            let ledger_before_failure = snapshot_files(ledger_directory.path());
            let body_before_failure = snapshot_files(body_directory.path());
            let payload_before_failure = snapshot_files(payload_directory.path());
            let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
                panic!("corrupt all-row Fetch census must fail closed")
            };
            assert!(matches!(
                error.kind,
                ProductionLifecycleStartupErrorKindV1::InvalidStorageCut
            ));
            assert_eq!(
                snapshot_files(ledger_directory.path()),
                ledger_before_failure
            );
            assert_eq!(snapshot_files(body_directory.path()), body_before_failure);
            assert_eq!(
                snapshot_files(payload_directory.path()),
                payload_before_failure
            );
        }

        #[test]
        fn production_owner_rejects_an_unsupported_live_class_before_publication() {
            let fixture = RecoveryFixture::new("unsupported-live-production-owner", 0x81);
            let replay = super::super::super::replay_authority::exact_record_fixture(
                fixture.lifecycle_context(),
                LifecycleStageKind::SignProposal,
                0x82,
            );
            let causal_root = CausalRoot::new(LifecycleDigest::new([0x83; 32]));
            let record = LifecycleLedgerRecordV1::new(
                replay.key,
                OwnerId::new(causal_root, 1),
                1,
                replay.work_class,
                replay.stage,
                None,
                causal_root.digest(),
                replay.payload,
                replay.authority,
                DurableContinuation::None,
            )
            .expect("construct unsupported live SignProposal row");
            let ledger = fixture.ledger(vec![record]);
            let body_directory = TempDir::new().expect("temporary unsupported-live body root");
            let body_store = fixture.open_store(&body_directory);
            let payload_directory =
                TempDir::new().expect("temporary unsupported-live payload root");
            let (payload_store, payloads) =
                fixture.open_empty_serve_payloads(&payload_directory, &body_store);
            let ledger_directory = TempDir::new().expect("temporary unsupported-live ledger root");
            let ledger_store = fixture.persist_ledger(&ledger_directory, &ledger);
            let cut = ledger
                .into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    ledger_store,
                    body_store,
                )
                .expect("seal unsupported-live storage cut before exhaustive classification");
            let before = (
                snapshot_files(ledger_directory.path()),
                snapshot_files(body_directory.path()),
                snapshot_files(payload_directory.path()),
            );
            let Err(error) = cut.open_owner_for_test(payload_store, payloads) else {
                panic!("unsupported live class must fail closed")
            };
            assert!(matches!(
                error.kind,
                ProductionLifecycleStartupErrorKindV1::Recovery(_)
            ));
            assert_eq!(snapshot_files(ledger_directory.path()), before.0);
            assert_eq!(snapshot_files(body_directory.path()), before.1);
            assert_eq!(snapshot_files(payload_directory.path()), before.2);
        }

        #[test]
        fn consuming_storage_cut_rejects_foreign_context_store_sources_and_qc() {
            let fixture = RecoveryFixture::new("durable-ready-fetch-rejections", 0x51);
            let foreign = RecoveryFixture::new("foreign-durable-ready-fetch", 0x61);

            let exact_empty_ledger = fixture.ledger(Vec::new());
            let exact_empty_body_directory =
                TempDir::new().expect("temporary exact empty body store");
            let exact_empty_body_store = fixture.open_store(&exact_empty_body_directory);
            let foreign_ledger = foreign.ledger(Vec::new());
            let foreign_ledger_directory =
                TempDir::new().expect("temporary foreign lifecycle ledger store");
            let foreign_ledger_store =
                foreign.persist_ledger(&foreign_ledger_directory, &foreign_ledger);
            assert!(matches!(
                exact_empty_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    foreign_ledger_store,
                    exact_empty_body_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidLedgerStore)
            ));

            let foreign_context_directory =
                TempDir::new().expect("temporary foreign-context body store");
            let mut foreign_context_store = fixture.open_store(&foreign_context_directory);
            let foreign_context_record =
                fixture.fetch_record(&mut foreign_context_store, 0, 0x71, 1, None, false);
            let foreign_context_ledger = fixture.ledger(vec![foreign_context_record]);
            let foreign_context_ledger_directory =
                TempDir::new().expect("temporary foreign-context lifecycle ledger");
            let foreign_context_ledger_store =
                fixture.persist_ledger(&foreign_context_ledger_directory, &foreign_context_ledger);
            assert!(matches!(
                foreign_context_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    foreign.verified.clone(),
                    foreign_context_ledger_store,
                    foreign_context_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidVerifiedContext)
            ));

            let foreign_store_directory =
                TempDir::new().expect("temporary exact-context body store");
            let mut exact_store = fixture.open_store(&foreign_store_directory);
            let exact_record = fixture.fetch_record(&mut exact_store, 0, 0x72, 1, None, false);
            let foreign_body_directory =
                TempDir::new().expect("temporary foreign body-store context");
            let foreign_store = foreign.open_store(&foreign_body_directory);
            let exact_ledger = fixture.ledger(vec![exact_record]);
            let exact_ledger_directory =
                TempDir::new().expect("temporary exact-context lifecycle ledger");
            let exact_ledger_store = fixture.persist_ledger(&exact_ledger_directory, &exact_ledger);
            assert!(matches!(
                exact_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    exact_ledger_store,
                    foreign_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidBodyStoreContext)
            ));

            let wrong_sources_directory =
                TempDir::new().expect("temporary wrong-sources body store");
            let mut wrong_sources_store = fixture.open_store(&wrong_sources_directory);
            let wrong_sources = vec![fixture.verified.context().roster[0].validator.clone()];
            let wrong_sources_record = fixture.fetch_record(
                &mut wrong_sources_store,
                0,
                0x73,
                1,
                Some(wrong_sources),
                false,
            );
            assert!(
                wrong_sources_record
                    .authenticate_durable_certified_fetch(&fixture.verified, || -> Result<
                        AuthenticatedDurableBodyFrameRecovery,
                        DurableBodyFrameRecoveryError,
                    > {
                        panic!("body-store authority must not be minted before source rejection")
                    })
                    .expect("source rejection does not inspect the body store")
                    .is_none()
            );
            let wrong_sources_ledger = fixture.ledger(vec![wrong_sources_record]);
            let wrong_sources_ledger_directory =
                TempDir::new().expect("temporary wrong-sources lifecycle ledger");
            let wrong_sources_ledger_store =
                fixture.persist_ledger(&wrong_sources_ledger_directory, &wrong_sources_ledger);
            assert!(matches!(
                wrong_sources_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    wrong_sources_ledger_store,
                    wrong_sources_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidReplayJoin)
            ));

            let corrupt_qc_directory = TempDir::new().expect("temporary corrupt-QC body store");
            let mut corrupt_qc_store = fixture.open_store(&corrupt_qc_directory);
            let corrupt_qc_record =
                fixture.fetch_record(&mut corrupt_qc_store, 0, 0x74, 1, None, true);
            let corrupt_qc_ledger = fixture.ledger(vec![corrupt_qc_record]);
            let corrupt_qc_ledger_directory =
                TempDir::new().expect("temporary corrupt-QC lifecycle ledger");
            let corrupt_qc_ledger_store =
                fixture.persist_ledger(&corrupt_qc_ledger_directory, &corrupt_qc_ledger);
            assert!(matches!(
                corrupt_qc_ledger.into_durable_certified_fetch_storage_recovery_cut(
                    fixture.verified.clone(),
                    corrupt_qc_ledger_store,
                    corrupt_qc_store,
                ),
                Err(DurableCertifiedFetchRecoveryError::InvalidReplayJoin)
            ));
        }
