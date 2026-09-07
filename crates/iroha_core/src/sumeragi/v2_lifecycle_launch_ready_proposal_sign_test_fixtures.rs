/// Prepared fallible ingress binding for the Ready local-Proposal Sign fixture.
pub(in crate::sumeragi) struct PreparedReadyLocalProposalSignIngressFixtureV1 {
    binding: ProductionLeaderWireIngressBindingV1,
}

impl LaunchedProductionLifecycleV1 {
    /// Prepare the isolated ingress binding before moving the live service owner.
    pub(in crate::sumeragi) fn prepare_ready_local_proposal_sign_ingress_for_test(
        executor: &V2EffectExecutor<SerializedV2Runtime>,
        directory: &TempDir,
        validator: &PeerId,
    ) -> PreparedReadyLocalProposalSignIngressFixtureV1 {
        let context_id = executor.context().id();
        let height = executor.context().height;
        let ingress = Arc::new(FairV2Ingress::new(16, 1 << 20, 1 << 18, 0, 0));
        ingress
            .configure_roster([validator.clone()])
            .expect("one-validator Ready Sign binding geometry");
        ingress.require_leader_wire_lifecycle_gate();
        ingress.state.lock().leader_wire_max_chunk_count = 2;
        let (gate, restore) = empty_leader_wire_gate_for_binding_test(
            directory,
            "ready-local-proposal-sign-leader-wire.wal",
            context_id,
            height,
            validator,
        );
        let leader_wire_ingress_binding = ProductionLeaderWireIngressBindingV1::bind(
            ingress,
            gate,
            restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            context_id,
            height,
        )
        .expect("bind the Ready local-Proposal Sign fixture ingress");
        PreparedReadyLocalProposalSignIngressFixtureV1 {
            binding: leader_wire_ingress_binding,
        }
    }

    /// Assemble the already-bound owner/executor/service triple used by the
    /// Ready local-Proposal Sign boundary regression.
    pub(in crate::sumeragi) fn ready_local_proposal_sign_fixture_for_test(
        owner: ProductionLifecycleOwnerV1,
        executor: V2EffectExecutor<SerializedV2Runtime>,
        services: ProductionV2Services,
        ingress: PreparedReadyLocalProposalSignIngressFixtureV1,
    ) -> Self {
        Self {
            owner,
            executor,
            services,
            pending_kura_apply_replay: None,
            recovered_local_proposal_attempt: None,
            pending_lifecycle_completion: None,
            pending_ingress_capacity: None,
            completion_observer_activation: None,
            leader_wire_ingress_binding: ingress.binding,
        }
    }

    /// Snapshot the serialized runtime without consuming its pending progress owner.
    pub(in crate::sumeragi) fn runtime_queue_snapshot_for_ready_sign_test(
        &self,
        now: Instant,
    ) -> crate::sumeragi::v2_runtime::RuntimeQueueSnapshot {
        self.executor.runtime_queue_snapshot_for_test(now)
    }

    /// Freeze the production timeout owner after a worker completion has
    /// become physical but before lifecycle publication consumes it.
    pub(in crate::sumeragi) fn freeze_due_timeout_for_ready_sign_test(
        &mut self,
        now: Instant,
    ) -> Result<bool, EffectExecutorError> {
        let physical_cut = self
            .leader_wire_ingress_binding
            .ingress
            .next_physical_admission_ordinal();
        self.executor
            .freeze_pre_timeout_locked_prepare_qc_cut(now, physical_cut)
            .map(|cut| cut.is_some())
    }

    /// Run one production executor step at an exact synthetic scheduler time.
    pub(in crate::sumeragi) fn step_runtime_for_ready_sign_test(
        &mut self,
        now: Instant,
    ) -> Result<crate::sumeragi::v2_effects::EffectExecutorStep, EffectExecutorError> {
        self.executor.step(now, &mut self.services)
    }

    /// Inspect the last production runtime owner selected by the executor.
    pub(in crate::sumeragi) fn runtime_step_observation_for_ready_sign_test(
        &self,
    ) -> Option<crate::sumeragi::v2_effects::RuntimeStepObservationV1> {
        self.executor.last_runtime_step_observation_for_test()
    }

    /// Return whether ProposalIntent fsync has produced its lifecycle Sign handoff.
    pub(in crate::sumeragi) fn has_pending_live_wal_sign_for_ready_sign_test(&self) -> bool {
        self.executor.has_pending_live_wal_sign_admission()
    }

    /// Retain one inert ordinary physical Completion head ahead of Ready work.
    pub(in crate::sumeragi) fn install_ordinary_completion_head_for_ready_sign_test(
        &mut self,
        planner: &crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
    ) {
        planner.publish_auxiliary_completion_fixture();
    }

    /// Return whether the inert ordinary physical Completion head remains retained.
    pub(in crate::sumeragi) fn ordinary_completion_head_retained_for_ready_sign_test(
        &self,
    ) -> bool {
        self.services.has_auxiliary_completion_head_for_test()
    }

    /// Drain only the retained ordinary physical head after Ready Sign dispatch.
    pub(in crate::sumeragi) fn drain_ordinary_completion_head_for_ready_sign_test(
        &mut self,
    ) -> Result<usize, EffectExecutorError> {
        self.services
            .drain_one_ordinary_completion_after_lifecycle_pass_through(&mut self.executor)
    }

    /// Execute the exact lifecycle Sign queued by the Ready-dispatch regression.
    pub(in crate::sumeragi) fn execute_ready_local_proposal_sign_for_test(
        &self,
        planner: &crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
        output_guard: Arc<ConsensusOutputGuard>,
    ) {
        planner.execute_one_recovered_lifecycle_sign_fixture(&self.services, output_guard);
    }

    /// Drive the exact-output corridor without allowing a Runtime turn.
    pub(in crate::sumeragi) fn retry_exact_output_for_ready_sign_test(
        &self,
    ) -> Result<bool, String> {
        self.services.retry_pending_exact_output()
    }

    /// Inspect whether the exact-output corridor still owns any fanout.
    pub(in crate::sumeragi) fn has_pending_exact_output_for_ready_sign_test(
        &self,
    ) -> Result<bool, String> {
        self.services.has_pending_exact_output()
    }

    /// Detach the synchronous worker fixture before dropping the launched shell.
    pub(in crate::sumeragi) fn detach_ready_sign_planner_for_test(
        &mut self,
        planner: crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
    ) {
        planner.detach(&mut self.services);
    }
}

impl LaunchedProductionLifecycleV1 {
    /// Transfer a genuine recovered Proposal owner into synchronous test I/O,
    /// retaining the production WAL gate, ordinal pair, and exact body instance.
    /// The enclosing worker test supplies its four-validator service fixture.
    #[inline(never)]
    pub(in crate::sumeragi) fn recovered_proposal_services_for_restart_test(
        mut owner: Box<ProductionLifecycleOwnerV1>,
        mut services: Box<ProductionV2Services>,
        wal_path: &std::path::Path,
        started_at: Instant,
        local_validator: wire::ValidatorIndex,
        output_guard: Arc<ConsensusOutputGuard>,
        ingress: Arc<FairV2Ingress>,
    ) -> (
        Box<Self>,
        Box<crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture>,
    ) {
        let context = owner.verified.context().clone();
        assert!(owner.exact_recovered_body_pipeline_join_for_test());
        let mut startup = owner
            .adapter_startup
            .take()
            .expect("retain the recovered adapter");
        let launch = startup
            .prepare_leader_wire_launch(wal_path)
            .expect("derive launch custody from the exact recovered safety WAL");
        let (runtime_authority, coordinator_authority) =
            super::super::authority::lifecycle_ordinal_authorities_after_high_watermark(
                owner.coordinator.high_water(),
            );
        let ordinals = RuntimeLifecycleOrdinalSource::from_authority(runtime_authority);
        if let Some(high_water) = launch.restored_producer_ordinal_high_watermark() {
            ordinals
                .advance_past(high_water)
                .expect("preserve recovered producer ordinals");
        }
        let (gate, restore, _recovery_authority) = launch
            .open_gate(
                &context,
                owner
                    .body_store
                    .as_ref()
                    .expect("retain exact recovered body store"),
            )
            .expect("open the genuine WAL-adjacent gate with its recovered body census");
        ordinals
            .advance_past(restore.scheduler_ordinal_high_watermark())
            .expect("preserve the restored scheduler high-water mark");
        owner
            .coordinator
            .bind_live_lifecycle_ordinal_authority(coordinator_authority)
            .expect("bind the same live ordinal cursor to the recovered registry");
        let (runtime, pending_kura_apply_replay, recovered_local_proposal_attempt) = startup
            .into_serialized_runtime(
                started_at,
                Duration::from_secs(2),
                RuntimeQueueConfig::new(8, 2, 2),
                ordinals.clone(),
            )
            .expect("consume the genuine recovered adapter into its runtime");
        assert!(pending_kura_apply_replay.is_none());
        assert!(recovered_local_proposal_attempt.is_some());
        let (executor, planner) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
            &mut services,
            runtime,
            output_guard,
            local_validator,
            4,
        );
        let binding = ProductionLeaderWireIngressBindingV1::bind(
            ingress,
            gate,
            restore,
            ordinals,
            context.id(),
            context.height,
        )
        .expect("bind the recovered WAL gate to the service's exact ingress instance");
        (
            Box::new(Self {
                owner: *owner,
                executor,
                services: *services,
                pending_kura_apply_replay,
                recovered_local_proposal_attempt,
                pending_lifecycle_completion: None,
                pending_ingress_capacity: None,
                completion_observer_activation: None,
                leader_wire_ingress_binding: binding,
            }),
            Box::new(planner),
        )
    }

    /// Borrow the already-paired restart fixture to assert real worker and
    /// lifecycle transitions without manufacturing a carrier or authority.
    #[inline(never)]
    pub(in crate::sumeragi) fn with_proposal_restart_fixture_for_test<R>(
        &mut self,
        inspect: impl FnOnce(
            &mut ProductionLifecycleOwnerV1,
            &mut V2EffectExecutor<SerializedV2Runtime>,
            &mut ProductionV2Services,
        ) -> R,
    ) -> R {
        inspect(&mut self.owner, &mut self.executor, &mut self.services)
    }
}
