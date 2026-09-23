// These tests exercise dispatch/custody with genuine frozen contexts and CommitQCs.
// TrackedOwner is only a cache ownership probe, never State resource admission.
mod retained_lifecycle_apply_tests {
    use super::*;
    use crate::sumeragi::{
        v2_apply::{
            retained_lifecycle_apply::RetainedLifecycleApplyError,
            validation_custody::{
                CarrierCustodyError, CarrierValidator, RetainedBodyValidationService,
                test_support::TrackedOwner,
            },
        },
        v2_body_store::{BodyValidationBusy, LocalValidationRefusal},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RetainedProbe {
        commitment: wire::ExecutionCommitment,
        executions: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
    }
    impl CarrierValidator for RetainedProbe {
        type Owner = TrackedOwner;
        type Error = LocalValidationRefusal;

        fn prepare(
            &mut self,
            context: &wire::HeightContext,
            body: &SignedBlock,
        ) -> Result<Self::Owner, Self::Error> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            Ok(TrackedOwner::new(
                context,
                body,
                self.commitment,
                Arc::clone(&self.drops),
            ))
        }

        fn resume(
            &mut self,
            _owner: Self::Owner,
        ) -> Result<Self::Owner, (Self::Owner, LocalValidationRefusal)> {
            panic!("ready original execution must not be resumed or replaced")
        }
    }

    struct RetainedFixture {
        store: V2BodyStore,
        service: RetainedBodyValidationService<RetainedProbe>,
        receipt: ValidatedBodyReceipt,
        executions: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
        _directory: tempfile::TempDir,
    }

    impl RetainedFixture {
        fn new(fixture: &ApplyFixture) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let mut store = V2BodyStore::open_with_policy(
                directory.path(),
                fixture.context.clone(),
                BlockSignaturePolicy::GenesisAuthority(
                    fixture
                        .service
                        .genesis_account
                        .expect_single_signatory()
                        .clone(),
                ),
            )
            .unwrap();
            let durable = store
                .store(
                    fixture.manifest.clone(),
                    fixture.body.encode_wire().unwrap(),
                )
                .unwrap();
            let executions = Arc::new(AtomicUsize::new(0));
            let drops = Arc::new(AtomicUsize::new(0));
            let budget = mv::allocation::AllocationBudget::new(
                store
                    .retained_validation_descriptor_bytes::<RetainedProbe>()
                    .unwrap(),
            );
            let mut service = store
                .retained_validation_service(
                    RetainedProbe {
                        commitment: fixture.task.certificate().execution_commitment,
                        executions: Arc::clone(&executions),
                        drops: Arc::clone(&drops),
                    },
                    &budget,
                )
                .unwrap();
            let receipt = store
                .execute_retained_durable_validation(
                    durable.clone(),
                    durable.manifest_hash(),
                    &mut service,
                )
                .unwrap()
                .into_validated_receipt()
                .unwrap();
            Self {
                store,
                service,
                receipt,
                executions,
                drops,
                _directory: directory,
            }
        }

        fn task(&self, fixture: &ApplyFixture) -> LifecycleDecisionApplyTaskV1 {
            let key = LifecycleDecisionApplyDispatchKeyV1::for_height_context_test(
                &fixture.context,
                17,
                42,
            );
            LifecycleDecisionApplyTaskV1::from_recovered_registry_projection(
                LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(key),
                EventTag::new(
                    fixture.context.height,
                    fixture.task.certificate().round.view,
                    Generation::new(1),
                ),
                fixture.task.subject(),
                fixture.task.certificate().clone(),
                self.receipt.clone(),
            )
            .unwrap()
        }

        fn assert_retained(&self, allocation: *const u64) {
            assert_eq!(self.executions.load(Ordering::SeqCst), 1);
            assert_eq!(self.drops.load(Ordering::SeqCst), 0);
            assert_eq!(self.service.marker_counts_for_test(), (0, 1));
            assert_eq!(
                self.service
                    .owner_for_test(self.receipt.durable().subject())
                    .unwrap()
                    .allocation(),
                allocation
            );
        }
    }

    #[test]
    fn retained_lifecycle_apply_preserves_original_dispatch_execution_and_release_on_retry() {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let mut retained = RetainedFixture::new(&fixture);
        let mut task = retained.task(&fixture);
        let dispatch = task.dispatch_key();
        let allocation = retained
            .service
            .owner_for_test(task.subject())
            .unwrap()
            .allocation();
        let state_height = fixture.state.committed_height();
        let release = concread::release::ReleaseNotification::default();
        let wait = release.observe();
        for _ in 0..3 {
            let refusal = fixture
                .service
                .publish_retained_lifecycle_decision_apply::<_, (), (), (), _>(
                    &fixture.context,
                    &mut retained.store,
                    &mut retained.service,
                    task,
                    |producer, owner, finality| {
                        assert_eq!(producer.executions.load(Ordering::SeqCst), 1);
                        assert_eq!(owner.allocation(), allocation);
                        assert_eq!(finality.artifact().commit_qc, *fixture.task.certificate());
                        Err((
                            owner,
                            LocalValidationRefusal::PhysicalBusy(BodyValidationBusy::new(
                                "retained publication fixture",
                                wait.clone(),
                                std::task::Waker::noop().clone(),
                            )),
                        ))
                    },
                )
                .err()
                .expect("local publisher refusal retains ownership");
            let RetainedLifecycleApplyError::Publication(LocalValidationRefusal::PhysicalBusy(
                busy,
            )) = refusal.error
            else {
                panic!("original publisher refusal must survive the handoff");
            };
            assert_eq!(busy.wait, wait);
            assert_eq!(refusal.task.dispatch_key(), dispatch);
            assert_eq!(refusal.task.validated_receipt(), &retained.receipt);
            retained.assert_retained(allocation);
            assert_eq!(fixture.state.committed_height(), state_height);
            // The exact durable marker is reusable only by this original owner.
            let same_receipt = retained
                .store
                .execute_retained_durable_validation(
                    retained.receipt.durable().clone(),
                    retained.receipt.durable().manifest_hash(),
                    &mut retained.service,
                )
                .unwrap()
                .into_validated_receipt()
                .unwrap();
            assert_eq!(same_receipt, retained.receipt);
            task = refusal.task;
        }
        drop(retained.service);
        assert_eq!(retained.drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn retained_lifecycle_apply_rejects_forged_or_rebound_task_before_selecting_original_owner() {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let mut retained = RetainedFixture::new(&fixture);
        let allocation = retained
            .service
            .owner_for_test(retained.receipt.durable().subject())
            .unwrap()
            .allocation();
        for fault in 0..7 {
            let mut task = retained.task(&fixture);
            match fault {
                0 => task.certificate.aggregate_signature[0] ^= 1,
                1 => {
                    task.dispatch_identity =
                        LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(
                            LifecycleDecisionApplyDispatchKeyV1::for_test(17, 42),
                        )
                }
                2 => {
                    task.lineage = LifecycleDecisionApplyTaskLineageV1::Live {
                        tag: task.exact_tag(),
                    }
                }
                3 => {
                    task.lineage = LifecycleDecisionApplyTaskLineageV1::Recovered {
                        tag: EventTag::new(fixture.context.height + 1, 0, Generation::new(1)),
                    }
                }
                4 => task.certificate.proposal_round.view += 1,
                5 => task.subject.payload_hash = Hash::new(b"foreign retained payload"),
                _ => {
                    task.dispatch_identity =
                        LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(
                            LifecycleDecisionApplyDispatchKeyV1::for_height_context_test(
                                &fixture.context,
                                0,
                                42,
                            ),
                        )
                }
            }
            let dispatch = task.dispatch_key();
            let refusal = fixture
                .service
                .publish_retained_lifecycle_decision_apply::<_, (), (), (), ()>(
                    &fixture.context,
                    &mut retained.store,
                    &mut retained.service,
                    task,
                    |_, _, _| panic!("malformed task must not enter the consuming publisher"),
                )
                .err()
                .expect("invalid task must retain the original dispatch");
            assert!(matches!(
                refusal.error,
                RetainedLifecycleApplyError::Authentication(_)
            ));
            assert_eq!(refusal.task.dispatch_key(), dispatch);
            retained.assert_retained(allocation);
        }
    }

    #[test]
    fn retained_lifecycle_apply_rejects_foreign_store_and_scalar_marker_substitution() {
        let fixture = ApplyFixture::new_for_production_recovered_decision_apply();
        let mut retained = RetainedFixture::new(&fixture);
        let allocation = retained
            .service
            .owner_for_test(retained.receipt.durable().subject())
            .unwrap()
            .allocation();
        // The original store holds an exclusive file lease. A second opening
        // of that same path must fail at the OS boundary before this custody
        // test can reach its identity check. Give the foreign store identical
        // authenticated bytes under a different lease instead.
        let foreign_directory = tempfile::tempdir().unwrap();
        let mut foreign = V2BodyStore::open_with_policy(
            foreign_directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(
                fixture
                    .service
                    .genesis_account
                    .expect_single_signatory()
                    .clone(),
            ),
        )
        .unwrap();
        let foreign_durable = foreign
            .store(fixture.manifest.clone(), fixture.body.encode_wire().unwrap())
            .unwrap();
        assert_eq!(foreign_durable.subject(), retained.receipt.durable().subject());
        let task = retained.task(&fixture);
        let refusal = fixture
            .service
            .publish_retained_lifecycle_decision_apply::<_, (), (), (), ()>(
                &fixture.context,
                &mut foreign,
                &mut retained.service,
                task,
                |_, _, _| panic!("another open store cannot borrow original execution"),
            )
            .err()
            .unwrap();
        assert!(matches!(
            refusal.error,
            RetainedLifecycleApplyError::Custody(CarrierCustodyError::Identity)
        ));
        retained.assert_retained(allocation);

        let budget = mv::allocation::AllocationBudget::new(
            retained
                .store
                .retained_validation_descriptor_bytes::<RetainedProbe>()
                .unwrap(),
        );
        let mut replacement = retained
            .store
            .retained_validation_service(
                RetainedProbe {
                    commitment: retained.receipt.execution_commitment(),
                    executions: Arc::clone(&retained.executions),
                    drops: Arc::clone(&retained.drops),
                },
                &budget,
            )
            .unwrap();
        let refusal = fixture
            .service
            .publish_retained_lifecycle_decision_apply::<_, (), (), (), ()>(
                &fixture.context,
                &mut retained.store,
                &mut replacement,
                refusal.task,
                |_, _, _| panic!("persisted scalar marker cannot replace the retained owner"),
            )
            .err()
            .unwrap();
        assert!(matches!(
            refusal.error,
            RetainedLifecycleApplyError::Custody(CarrierCustodyError::Unconfirmed)
        ));
        retained.assert_retained(allocation);
    }
}
