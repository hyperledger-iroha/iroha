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
