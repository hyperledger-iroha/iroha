// Included in block::tests; exercise the real block pipeline, not a local permission substitute.

std::thread_local! {
    static PARALLEL_ACCOUNT_PROFILE_TEST_SCOPE: std::cell::RefCell<
        Option<iroha_data_model::account::address::ChainDiscriminantGuard>
    > = const { std::cell::RefCell::new(None) };
}

struct ParallelAccountProfilePoolScope {
    pool: Arc<rayon::ThreadPool>,
}

impl ParallelAccountProfilePoolScope {
    fn enter(pool: Arc<rayon::ThreadPool>, discriminant: u16) -> Self {
        pool.broadcast(|_| {
            PARALLEL_ACCOUNT_PROFILE_TEST_SCOPE.with(|slot| {
                let mut slot = slot.borrow_mut();
                assert!(
                    slot.is_none(),
                    "fresh fixture pool must have no retained scope"
                );
                *slot = Some(
                    iroha_data_model::account::address::ChainDiscriminantGuard::enter(discriminant),
                );
            });
        });
        Self { pool }
    }

    fn assert_profile(&self, expected: u16) {
        assert_eq!(self.pool.current_num_threads(), 2);
        let profiles = self
            .pool
            .broadcast(|_| iroha_data_model::account::address::chain_discriminant());
        assert_eq!(profiles, vec![expected; 2]);
    }
}

impl Drop for ParallelAccountProfilePoolScope {
    fn drop(&mut self) {
        self.pool.broadcast(|_| {
            PARALLEL_ACCOUNT_PROFILE_TEST_SCOPE.with(|slot| {
                // Drop each guard on its owning worker; never mutate the process default.
                slot.borrow_mut().take();
            });
        });
    }
}

struct ParallelAccountProfileFixture {
    authority: AccountId,
    signer: KeyPair,
    targets: [AccountId; 2],
    previous: SignedBlock,
}

impl ParallelAccountProfileFixture {
    fn new() -> Self {
        let (authority, signer) = gen_account_in("wonderland");
        let targets = [
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
        ];
        assert!(targets.iter().all(|target| target != &authority));
        assert_ne!(targets[0], targets[1]);
        Self {
            authority,
            signer,
            targets,
            previous: previous_block_at_height(1),
        }
    }

    fn execute(
        &self,
        permission_discriminant: u16,
        worker_discriminant: Option<u16>,
    ) -> (Vec<TransactionResultInner>, Vec<Option<Json>>) {
        use iroha_data_model::{
            account::address::{ChainDiscriminantGuard, chain_discriminant},
            permission::{Permission, Permissions},
        };
        use iroha_executor_data_model::permission::account::CanModifyAccountMetadata;

        let _profile = ChainDiscriminantGuard::enter(369);
        let _status = crate::sumeragi::status::rbc_status_test_guard();
        let permissions = {
            let _permission_profile = ChainDiscriminantGuard::enter(permission_discriminant);
            self.targets
                .iter()
                .map(|account| {
                    Permission::from(CanModifyAccountMetadata {
                        account: account.clone(),
                    })
                })
                .collect::<Vec<_>>()
        };
        for (permission, target) in permissions.iter().zip(&self.targets) {
            let decoded = CanModifyAccountMetadata::try_from(permission);
            if permission_discriminant == 369 {
                assert_eq!(
                    decoded.expect("selected permission profile").account,
                    *target
                );
            } else {
                assert!(
                    decoded.is_err(),
                    "foreign I105 payload must fail native decoding"
                );
            }
        }
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain");
        let domain = Domain::new(domain_id).build(&self.authority);
        let accounts = std::iter::once(&self.authority)
            .chain(self.targets.iter())
            .map(|account| Account::new(account.clone()).build(account));
        let mut world = World::with([domain], accounts, []);
        world
            .account_permissions_mut_for_testing()
            .insert(self.authority.clone(), Permissions::from_iter(permissions));
        let mut state = State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            ChainId::from("fc56984b-2be7-431d-840e-21514d1883f0"),
            deterministic_test_network_id(0x0B),
        );
        install_test_lane_manifests(&state);
        let mut pipeline = state.pipeline.clone();
        pipeline.workers = if worker_discriminant.is_some() { 2 } else { 1 };
        pipeline.parallel_overlay = worker_discriminant.is_some();
        pipeline.parallel_apply = worker_discriminant.is_some();
        state.set_pipeline(pipeline);
        finalize_test_genesis_assets(&state, &self.previous);

        let key: Name = "delegated_profile_marker".parse().expect("metadata key");
        let accepted = self
            .targets
            .iter()
            .enumerate()
            .map(|(index, target)| {
                let mut transaction = TransactionBuilder::new(
                    state.network_id,
                    self.authority.clone(),
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                );
                transaction.set_creation_time(Duration::from_millis(
                    u64::try_from(index).expect("two transactions"),
                ));
                let signed = transaction
                    .with_instructions([SetKeyValue::account(
                        target.clone(),
                        key.clone(),
                        Json::new("delegated under 369"),
                    )])
                    .sign(self.signer.private_key());
                AcceptedTransaction::new_unchecked(Cow::Owned(signed))
            })
            .collect();
        let (_clock, time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let block = BlockBuilder::new_with_time_source(accepted, time_source)
            .chain(1, Some(&self.previous))
            .sign(self.signer.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());
        let pool_scope = worker_discriminant.map(|discriminant| {
            let pool = state_block
                .pipeline_thread_pool()
                .expect("configured native pool");
            let scope = ParallelAccountProfilePoolScope::enter(pool, discriminant);
            scope.assert_profile(discriminant);
            scope
        });
        assert_eq!(
            chain_discriminant(),
            369,
            "worker setup must not alter the caller"
        );
        let valid = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results = valid
            .as_ref()
            .entrypoint_results()
            .map(|(_, _, result)| result.0.clone())
            .collect::<Vec<_>>();
        let values = self
            .targets
            .iter()
            .map(|target| {
                state_block
                    .world
                    .map_account(target, |account| {
                        account.value().metadata().get(&key).cloned()
                    })
                    .expect("target account remains present")
            })
            .collect::<Vec<_>>();
        let execution = crate::sumeragi::status::snapshot().pipeline_execution;
        assert_eq!(
            execution.tx_vertices_total, 2,
            "exact fixture pipeline snapshot"
        );
        if let Some(scope) = pool_scope.as_ref() {
            scope.assert_profile(worker_discriminant.expect("parallel profile"));
            assert_eq!(
                execution.detached_prepared_total, 2,
                "real detached path must run"
            );
            assert_eq!(execution.detached_merged_total, 0);
            assert_eq!(execution.detached_fallback_total, 2);
            // SetKeyValue requires live authorization, even after a successful
            // worker check. These counters distinguish that deliberate fallback
            // from a profile-dependent worker rejection; final results alone cannot.
            if permission_discriminant == 369 {
                assert_eq!(
                    execution.detached_fallback_unsupported_instruction_total, 2,
                    "correct worker authorization reaches the mandatory live fallback"
                );
                assert_eq!(execution.detached_fallback_rejected_eval_total, 0);
            } else {
                assert_eq!(
                    execution.detached_fallback_rejected_eval_total, 2,
                    "worker must reject a foreign token before live revalidation"
                );
                assert_eq!(execution.detached_fallback_unsupported_instruction_total, 0);
            }
        } else {
            assert_eq!(
                execution.detached_prepared_total, 0,
                "serial comparison path"
            );
        }
        assert_eq!(
            chain_discriminant(),
            369,
            "execution must restore caller profile"
        );
        drop(pool_scope);
        (results, values)
    }
}

#[test]
fn parallel_account_profile_preserves_delegated_metadata_results() {
    let fixture = ParallelAccountProfileFixture::new();
    let serial = fixture.execute(369, None);
    assert!(
        serial.0.iter().all(Result::is_ok),
        "serial delegation must succeed: {:?}",
        serial.0
    );
    assert_eq!(serial.1, vec![Some(Json::new("delegated under 369")); 2]);
    for worker_profile in [753, 777] {
        let parallel = fixture.execute(369, Some(worker_profile));
        assert_eq!(
            parallel, serial,
            "worker profile {worker_profile} changed native execution"
        );
    }
}

#[test]
fn parallel_account_profile_rejects_foreign_permission_payloads() {
    let fixture = ParallelAccountProfileFixture::new();
    for foreign_profile in [753, 777] {
        let serial = fixture.execute(foreign_profile, None);
        assert!(
            serial.0.iter().all(|result| matches!(
                result,
                Err(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted(_)
                ))
            )),
            "foreign permission must reject for its native policy reason: {:?}",
            serial.0
        );
        assert_eq!(serial.1, vec![None; 2]);
        let parallel = fixture.execute(foreign_profile, Some(foreign_profile));
        assert_eq!(
            parallel, serial,
            "worker profile must not authorize a foreign permission"
        );
    }
}
