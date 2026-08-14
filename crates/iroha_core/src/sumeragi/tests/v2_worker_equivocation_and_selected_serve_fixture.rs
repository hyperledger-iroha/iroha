#[cfg(feature = "bls")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectedServeTimeoutRecoveryMode {
    TimeoutRecovery,
    LatePassiveFetch,
}
#[cfg(feature = "bls")]
struct SelectedServeLatePassiveFetch {
    body_store: V2BodyStore,
    task: BodyFetchTask,
    manifest: wire::PayloadManifest,
    body: Vec<u8>,
}
/// Build exact signed phase-vote evidence for the production persistence bridge.
fn exact_vote_equivocation(
    service: &ProductionV2Services,
    keys: &[KeyPair],
) -> wire::SumeragiV2Equivocation {
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let signer = 1;
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"equivocation parent state"),
        Hash::new(b"equivocation post state"),
        Hash::new(b"equivocation ordinary writes"),
        1,
        Hash::new(b"equivocation executed block"),
    );
    let signed_vote = |seed: u8| {
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])),
                payload_hash: Hash::prehashed([seed.wrapping_add(1); 32]),
            },
            execution_commitment,
            signer,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(signer).expect("small signer index")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        vote
    };
    wire::SumeragiV2Equivocation::PhaseVote {
        first: signed_vote(0xA1),
        second: signed_vote(0xA2),
    }
}
#[test]
fn production_equivocation_bridge_validates_persists_and_deduplicates_restart_replay() {
    let (mut service, keys) = fixture();
    let evidence = exact_vote_equivocation(&service, &keys);
    service
        .report_equivocation(evidence.clone())
        .expect("persist valid exact equivocation evidence");
    let shared_state = Arc::clone(&service.state);
    assert_eq!(
        shared_state.world.consensus_evidence.view().iter().count(),
        1
    );
    let wire::SumeragiV2Equivocation::PhaseVote { first, second } = evidence.clone() else {
        unreachable!("phase-vote fixture")
    };
    service
        .report_equivocation(wire::SumeragiV2Equivocation::PhaseVote {
            first: second,
            second: first,
        })
        .expect("swapped replay is an idempotent duplicate");
    let (mut restarted_service, _) = fixture();
    restarted_service.context = service.context.clone();
    restarted_service.validator_set_pops = service.validator_set_pops.clone();
    restarted_service.state = Arc::clone(&shared_state);
    restarted_service
        .report_equivocation(evidence)
        .expect("restart replay observes the canonical persisted key");
    assert_eq!(
        shared_state.world.consensus_evidence.view().iter().count(),
        1
    );
}
#[test]
fn production_equivocation_bridge_rejects_invalid_or_unanchored_evidence() {
    let (mut invalid_service, invalid_keys) = fixture();
    let mut forged = exact_vote_equivocation(&invalid_service, &invalid_keys);
    let wire::SumeragiV2Equivocation::PhaseVote { second, .. } = &mut forged else {
        unreachable!("phase-vote fixture")
    };
    second.signature[0] ^= 0x80;
    assert!(
        invalid_service.report_equivocation(forged).is_err(),
        "invalid evidence must fail before persistence or reporting"
    );
    assert_eq!(
        invalid_service
            .state
            .world
            .consensus_evidence
            .view()
            .iter()
            .count(),
        0
    );
    let (mut foreign_context_service, foreign_keys) = fixture();
    foreign_context_service.context.network_id =
        crate::sumeragi::synthetic_network_id("foreign-evidence-chain");
    let foreign_evidence = exact_vote_equivocation(&foreign_context_service, &foreign_keys);
    assert!(
        foreign_context_service
            .report_equivocation(foreign_evidence)
            .is_err(),
        "a valid pair from an unanchored context must fail closed"
    );
    assert_eq!(
        foreign_context_service
            .state
            .world
            .consensus_evidence
            .view()
            .iter()
            .count(),
        0
    );
}
/// Production-shaped selected-Serve recovery shared with the runner regression.
#[cfg(feature = "bls")]
pub(in crate::sumeragi) struct SelectedServeTimeoutRecoveryFixture {
    _runtime_directory: TempDir,
    _leader_wire_directory: TempDir,
    ingress: Arc<FairV2Ingress>,
    serve_gate: CertifiedServeIngressGate,
    missing_proposal_request: AuthenticatedCertifiedBodyRequest,
    missing_proposal_request_hash: HashOf<wire::CertifiedBodyRequest>,
    late_passive_fetch: Option<SelectedServeLatePassiveFetch>,
    executor: V2EffectExecutor<SerializedV2Runtime>,
    services: ProductionV2Services,
    command_rx: V2IoCommandReceiver,
    completion_tx: mpsc::SyncSender<V2IoCompletion>,
    completion_admission: Arc<V2IoAdmission>,
    local_key: KeyPair,
    consensus_observations: Arc<Mutex<Vec<ConsensusRouteObservation>>>,
    remote_timeout_votes_admitted: usize,
    timeout_prefix_completions: usize,
    local_timeout_signature_completed: bool,
}
#[cfg(feature = "bls")]
impl SelectedServeTimeoutRecoveryFixture {
    /// Build one missing-body Serve barrier followed by two authenticated timeout votes.
    pub(in crate::sumeragi) fn new() -> Self {
        Self::new_for_mode(SelectedServeTimeoutRecoveryMode::TimeoutRecovery)
    }
    /// Build one passive Fetch before the selected missing-body Serve barrier.
    pub(in crate::sumeragi) fn new_late_passive_fetch() -> Self {
        Self::new_for_mode(SelectedServeTimeoutRecoveryMode::LatePassiveFetch)
    }
    #[allow(clippy::too_many_lines)]
    fn new_for_mode(mode: SelectedServeTimeoutRecoveryMode) -> Self {
        let (mut services, keys) = fixture();
        if mode == SelectedServeTimeoutRecoveryMode::LatePassiveFetch {
            allow_fixture_block_payload(&mut services.context);
            services.leader_wire_recovery_authority = super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                    services.context.id(),
                    services.context.height,
                    [0xF4; 32],
                    services.active_tag.view(),
                    false,
                );
        }
        let context = services.context.clone();
        assert_eq!(
            context.roster.len(),
            4,
            "selected-Serve timeout recovery requires four representative validators"
        );
        let view_zero_leader = context.leader(0);
        let local_validator = (0..context.roster.len())
            .map(|index| u32::try_from(index).expect("fixture roster index fits u32"))
            .find(|index| *index != view_zero_leader)
            .expect("four-validator fixture has a non-leader timeout signer");
        let local_index =
            usize::try_from(local_validator).expect("fixture local validator fits usize");
        let local_key = keys[local_index].clone();
        services.local_validator = Some(local_validator);
        services.local_peer = context.roster[local_index].validator.clone();
        services.key_pair = local_key.clone();
        let (command_tx, command_rx, admission) = test_io_command_channel(8);
        let lifecycle_ordinals = command_tx.queue.lifecycle_ordinals.clone();
        let completion_admission = Arc::clone(&admission);
        let (completion_tx, completion_rx) = mpsc::sync_channel(8);
        services.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        let serve_gate = services
            .io
            .as_ref()
            .expect("install the manual production I/O boundary")
            .certified_serve_ingress_gate();
        let ingress = Arc::new(
            FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                128,
                512 * 1024 * 1024,
                64 * 1024 * 1024,
                super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
                8 * 1024 * 1024,
                8 * 1024 * 1024,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                None,
            ),
        );
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        ingress
            .configure_roster_for_context(
                roster.iter().cloned(),
                &context.network_id,
                context.da_layout,
            )
            .expect("configure selected-Serve timeout ingress");
        ingress.require_certified_serve_gate();
        ingress.require_leader_wire_lifecycle_gate();
        ingress
            .bind_certified_serve_gate(serve_gate.clone())
            .expect("bind the production Serve gate");
        let leader_wire_directory =
            TempDir::new().expect("temporary selected-Serve leader-wire directory");
        let capacity =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
                roster.len(),
                context.da_layout.max_chunk_count,
            )
            .expect("derive selected-Serve leader-wire capacity");
        let recovery_authority = services.leader_wire_recovery_authority;
        let (leader_wire_gate, restore) =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
                &leader_wire_directory
                    .path()
                    .join("selected-serve-timeout-recovery.wal"),
                context.id(),
                context.height,
                [0xF4; 32],
                roster,
                capacity,
                context.da_layout.max_chunk_count,
                recovery_authority,
                &[],
                &[],
            )
            .expect("open selected-Serve leader-wire gate");
        ingress
            .bind_leader_wire_lifecycle_gate(
                leader_wire_gate,
                restore,
                lifecycle_ordinals.clone(),
                context.id(),
                context.height,
            )
            .expect("bind the shared leader-wire lifecycle source");
        ingress.open().expect("open selected-Serve timeout ingress");
        services.leader_wire_ingress = Arc::clone(&ingress);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator proof of possession")
            })
            .collect();
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verify selected-Serve runtime context");
        let runtime_directory = TempDir::new().expect("temporary selected-Serve runtime directory");
        if mode == SelectedServeTimeoutRecoveryMode::LatePassiveFetch {
            services.chunk_root = runtime_directory.path().join("chunks");
        }
        let (adapter, startup_effects) = SumeragiV2Adapter::open(
            runtime_directory.path().join("selected-serve-runtime.wal"),
            verified,
            Some(local_validator),
            Generation::new(context.height),
            [0xF4; 32],
            AdapterFingerprints {
                node: Hash::new(b"selected Serve timeout node"),
                build: Hash::new(b"selected Serve timeout build"),
                config: Hash::new(b"selected Serve timeout config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open selected-Serve runtime adapter");
        assert!(startup_effects.is_empty());
        let round_timeout = match mode {
            SelectedServeTimeoutRecoveryMode::TimeoutRecovery => Duration::from_millis(1),
            SelectedServeTimeoutRecoveryMode::LatePassiveFetch => Duration::from_secs(24 * 60 * 60),
        };
        let started_at = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("fixture clock has a one-second predecessor");
        let (runtime, startup_effects) = SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup_effects,
            started_at,
            round_timeout,
            RuntimeQueueConfig::new(8, 2, 2),
            lifecycle_ordinals,
        )
        .expect("construct selected-Serve serialized runtime");
        assert!(startup_effects.is_empty());
        let mut executor = V2EffectExecutor::with_runtime(
            runtime,
            BTreeMap::new(),
            context.clone(),
            services.local_peer.clone(),
            Some(local_validator),
            EffectQueueConfig::default(),
        )
        .expect("construct selected-Serve effect executor");
        let late_passive_fetch = match mode {
            SelectedServeTimeoutRecoveryMode::TimeoutRecovery => {
                executor
                    .arm_live_clocks(started_at)
                    .expect("arm selected-Serve timeout clocks");
                let timeout_owner = executor
                    .freeze_due_timeout_owner_for_test(Instant::now())
                    .expect("freeze the height-start timeout before later Serve ingress");
                assert_eq!(
                    timeout_owner.lifecycle_ordinal(),
                    1,
                    "the height-start timeout owns the first actor-global scheduler position"
                );
                None
            }
            SelectedServeTimeoutRecoveryMode::LatePassiveFetch => {
                let late_dispatch_at = Instant::now();
                executor
                    .arm_live_clocks(late_dispatch_at)
                    .expect("arm non-due late-passive-Fetch clocks");
                let (body, payload, mut proposal) = proposal_body_and_payload(&context, &keys);
                let proposer_index =
                    usize::try_from(proposal.proposer).expect("fixture proposal index fits usize");
                proposal.signature = Signature::new(
                    keys[proposer_index].private_key(),
                    &proposal.signature_preimage(),
                )
                .payload()
                .to_vec();
                executor
                    .enqueue_network(wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::Proposal(proposal),
                    ))
                    .expect("enqueue the signed late-passive-Fetch proposal");
                assert!(matches!(
                    executor
                        .step(late_dispatch_at, &mut services)
                        .expect("dispatch the signed proposal into passive Fetch work"),
                    EffectExecutorStep::Advanced { .. }
                ));
                assert_eq!(
                    executor.status().pending_fetches,
                    1,
                    "the signed Proposal must establish reducer body-work ownership"
                );
                assert_eq!(
                    services.fetches.len(),
                    1,
                    "the passive Fetch must cross the production service boundary"
                );
                let task = services
                    .fetches
                    .values()
                    .next()
                    .expect("one production passive Fetch remains live")
                    .task
                    .clone();
                assert_eq!(task.manifest(), Some(payload.manifest()));
                let body_store =
                    V2BodyStore::open(runtime_directory.path().join("bodies"), context.clone())
                        .expect("open the retained late-passive-Fetch body store");
                Some(SelectedServeLatePassiveFetch {
                    body_store,
                    task,
                    manifest: payload.manifest().clone(),
                    body,
                })
            }
        };
        let consensus_observations = install_consensus_route_observer(&mut services);
        // Timeout mode freezes the height-start owner before later ingress.
        // Late-Fetch mode instead established its passive reducer owner
        // above. In both cases the selected Serve must take the next shared
        // actor-global position without jumping its predecessor.
        let missing_proposal_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let missing_proposal_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"selected Serve missing proposal",
            )),
            payload_hash: Hash::new(b"selected Serve missing proposal payload"),
        };
        let requester_index = (0..keys.len())
            .find(|index| *index != local_index)
            .expect("four-validator fixture has a remote Serve requester");
        let missing_request = authenticated_serve_request(
            &context,
            &keys[requester_index],
            missing_proposal_round,
            missing_proposal_subject,
            wire::GlobalPhase::Prepare,
        );
        let missing_proposal_request_hash = missing_request.request_hash();
        let authenticated_via = missing_request.request().requester.clone();
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(
                missing_request.request(),
                authenticated_via,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));
        if let Some(late_passive_fetch) = &late_passive_fetch {
            let barrier = serve_gate
                .selected_barrier()
                .expect("inspect late-passive-Fetch Serve barrier")
                .expect("late-passive-Fetch Serve remains selected");
            assert_eq!(
                barrier.scheduler_ordinal(),
                late_passive_fetch
                    .task
                    .lifecycle_ordinal()
                    .checked_add(1)
                    .expect("late passive Fetch ordinal has a successor"),
                "Serve admission must take the next shared actor-global ordinal"
            );
        }
        if mode == SelectedServeTimeoutRecoveryMode::TimeoutRecovery {
            let remote_signers = (0..keys.len())
                .filter(|index| *index != local_index)
                .take(2)
                .collect::<Vec<_>>();
            assert_eq!(remote_signers.len(), 2);
            for signer_index in remote_signers {
                let signer = u32::try_from(signer_index).expect("timeout signer fits u32");
                let mut timeout_vote = wire::TimeoutVote {
                    round: missing_proposal_round,
                    highest_prepare_qc: None,
                    signer,
                    signature: Vec::new(),
                };
                timeout_vote.signature = Signature::new(
                    keys[signer_index].private_key(),
                    &timeout_vote.signature_preimage(),
                )
                .payload()
                .to_vec();
                let source = context.roster[signer_index].validator.clone();
                assert!(matches!(
                    ingress.try_push(InboundBlockMessage::new(
                        BlockMessage::V2(wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::TimeoutVote(timeout_vote),
                        )),
                        Some(source),
                    )),
                    Ok(FairV2IngressPushDisposition::Enqueued)
                ));
            }
        }
        let fixture = Self {
            _runtime_directory: runtime_directory,
            _leader_wire_directory: leader_wire_directory,
            ingress,
            serve_gate,
            missing_proposal_request: missing_request,
            missing_proposal_request_hash,
            late_passive_fetch,
            executor,
            services,
            command_rx,
            completion_tx,
            completion_admission,
            local_key,
            consensus_observations,
            remote_timeout_votes_admitted: 0,
            timeout_prefix_completions: 0,
            local_timeout_signature_completed: false,
        };
        fixture.assert_missing_proposal_serve_selected();
        fixture
    }
    /// Service the production exact-Serve prefix before its liveness suffix.
    pub(in crate::sumeragi) fn service_exact_serve_runtime_prefix(
        &mut self,
    ) -> Result<bool, String> {
        let barrier = self
            .services
            .certified_serve_barrier()?
            .ok_or_else(|| "selected-Serve fixture lost its exact barrier".to_owned())?;
        let completion_evidence = self
            .services
            .certified_serve_predecessor_completion_evidence(
                self.executor.remaining_completion_capacity() != 0,
                barrier.scheduler_ordinal(),
            )?;
        if let Some(witness) = self
            .executor
            .exact_serve_predecessor_episode_witness(
                Instant::now(),
                barrier.scheduler_ordinal(),
                completion_evidence,
            )
            .map_err(|error| error.to_string())?
        {
            let _ = self
                .services
                .observe_certified_serve_predecessor_episode_witness(barrier, witness)?;
        }
        let claimed = self
            .services
            .claim_certified_serve_runtime_episode(barrier)?;
        if !claimed {
            self.assert_missing_proposal_serve_selected();
            return Ok(false);
        }
        let _ = self
            .services
            .drain_exact_serve_runtime_predecessor(&mut self.executor, barrier.scheduler_ordinal())
            .map_err(|error| error.to_string())?;
        let completion_evidence = self
            .services
            .certified_serve_predecessor_completion_evidence(
                self.executor.remaining_completion_capacity() != 0,
                barrier.scheduler_ordinal(),
            )?;
        let predecessor_witness = self
            .executor
            .exact_serve_predecessor_episode_witness(
                Instant::now(),
                barrier.scheduler_ordinal(),
                completion_evidence,
            )
            .map_err(|error| error.to_string())?;
        if let Some(witness) = predecessor_witness {
            let _ = self
                .services
                .observe_certified_serve_predecessor_episode_witness(barrier, witness)?;
        }
        if predecessor_witness.is_some()
            && self
                .services
                .certified_serve_runtime_predecessor_capacity_available(barrier)?
        {
            self.executor
                .set_ingress_physical_cut(self.ingress.next_physical_admission_ordinal())
                .map_err(|error| error.to_string())?;
            let _ = self
                .executor
                .step(Instant::now(), &mut self.services)
                .map_err(|error| error.to_string())?;
        }
        let completion_evidence = self
            .services
            .certified_serve_predecessor_completion_evidence(
                self.executor.remaining_completion_capacity() != 0,
                barrier.scheduler_ordinal(),
            )?;
        let predecessor_witness = self
            .executor
            .exact_serve_predecessor_episode_witness(
                Instant::now(),
                barrier.scheduler_ordinal(),
                completion_evidence,
            )
            .map_err(|error| error.to_string())?;
        if let Some(witness) = predecessor_witness {
            let _ = self
                .services
                .observe_certified_serve_predecessor_episode_witness(barrier, witness)?;
        }
        let older_predecessor_remains = predecessor_witness.is_some();
        self.services
            .finish_certified_serve_runtime_episode_turn(barrier, older_predecessor_remains)?;
        self.assert_missing_proposal_serve_selected();
        Ok(true)
    }
    /// Drive a late passive Fetch through Store and rejected validation, then release Serve.
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn assert_late_passive_fetch_completion_reopens_selected_serve(
        &mut self,
    ) {
        let mut late = self
            .late_passive_fetch
            .take()
            .expect("fixture owns one late passive Fetch");
        let fetch_ordinal = late.task.lifecycle_ordinal();
        assert!(
            self.service_exact_serve_runtime_prefix()
                .expect("complete the initially selected Serve predecessor episode")
        );
        assert!(
            !self
                .service_exact_serve_runtime_prefix()
                .expect("the passive Fetch alone cannot reopen the completed episode"),
            "transport-passive Fetch work is not runnable reducer progress"
        );
        assert_eq!(
            self.executor
                .complete_body_reconstruction(
                    &late.task,
                    late.manifest.clone(),
                    late.body.clone(),
                    &mut self.services,
                )
                .expect("complete the exact passive body reconstruction"),
            CompletionDisposition::Accepted
        );
        assert!(
            self.service_exact_serve_runtime_prefix()
                .expect("the late BodyAvailable successor reopens the Serve episode")
        );
        let store_task = match self.command_rx.try_recv() {
            Ok(V2IoCommand::Store(task)) => task,
            Ok(_) => panic!("late passive Fetch queued a non-Store command"),
            Err(error) => panic!("late passive Fetch omitted its Store command: {error}"),
        };
        assert_eq!(
            store_task.lifecycle_ordinal(),
            fetch_ordinal,
            "Store must retain the original passive Fetch owner"
        );
        assert!(
            !self
                .service_exact_serve_runtime_prefix()
                .expect("an incomplete Store cannot reopen the completed episode"),
            "active Store work remains passive until its tracked completion exists"
        );
        let stored = late
            .body_store
            .execute_store_task(&store_task)
            .expect("durably store the late reconstructed body");
        self.command_rx.complete_work(store_task.id());
        try_send_tracked_completion_with_lifecycle_ordinal(
            &self.completion_tx,
            &self.completion_admission,
            V2IoCompletion::Stored(stored),
            Some(fetch_ordinal),
        )
        .expect("deliver the exact tracked Store completion");
        assert!(
            self.service_exact_serve_runtime_prefix()
                .expect("the stored-body completion reopens and queues validation")
        );
        let validation_task = match self.command_rx.try_recv() {
            Ok(V2IoCommand::Validate(task)) => task,
            Ok(_) => panic!("late passive Fetch queued a non-Validate command"),
            Err(error) => {
                panic!("late passive Fetch omitted its Validate command: {error}")
            }
        };
        assert_eq!(
            validation_task.lifecycle_ordinal(),
            fetch_ordinal,
            "Validate must retain the original passive Fetch owner"
        );
        assert!(
            !self
                .service_exact_serve_runtime_prefix()
                .expect("an incomplete Validate cannot reopen the completed episode"),
            "active Validate work remains passive until its tracked completion exists"
        );
        let validated = late
            .body_store
            .execute_validation_task(&validation_task, |_| {
                Err::<wire::ExecutionCommitment, String>(
                    "deterministic late-passive-Fetch rejection".to_owned(),
                )
            })
            .expect("execute deterministic late-body validation");
        assert!(matches!(
            &validated,
            BodyValidationCompletion::Rejected { work_id, reason }
                if *work_id == validation_task.id()
                    && reason == "deterministic late-passive-Fetch rejection"
        ));
        self.command_rx.complete_work(validation_task.id());
        try_send_tracked_completion_with_lifecycle_ordinal(
            &self.completion_tx,
            &self.completion_admission,
            V2IoCompletion::Validated(validated),
            Some(fetch_ordinal),
        )
        .expect("deliver the exact tracked validation completion");
        assert!(
            self.service_exact_serve_runtime_prefix()
                .expect("the rejected validation retires its ValidationFailed successor")
        );
        assert!(
            !self
                .service_exact_serve_runtime_prefix()
                .expect("the retired body pipeline leaves no older predecessor"),
            "the rejected late body pipeline must terminate before Serve"
        );
        let requester = self.missing_proposal_request.request().requester.clone();
        let (admission, committed) = drain_and_commit_gated_serve(
            &self.ingress,
            &self
                .services
                .io
                .as_ref()
                .expect("late-passive-Fetch fixture retains its I/O service")
                .command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &self.missing_proposal_request,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert!(matches!(
            self.command_rx.try_recv(),
            Ok(V2IoCommand::Serve {
                lifecycle_id,
                request,
            }) if lifecycle_id == admission.lifecycle_id
                && request.request_hash() == self.missing_proposal_request_hash
        ));
        let producer_episode = self
            .services
            .try_begin_certified_serve_producer_episode()
            .expect("inspect producer ownership after exact Serve drain")
            .expect("the exact Serve completion must reopen one producer episode");
        assert!(
            self.services
                .try_begin_certified_serve_producer_episode()
                .is_err(),
            "one live producer lease must reject a nested ownership claim"
        );
        drop(producer_episode);
    }
    /// Admit at most one exact timeout-vote owner through the Serve-only bypass.
    pub(in crate::sumeragi) fn service_timeout_vote_episode(&mut self) -> Result<(), String> {
        let executor = &self.executor;
        let Some((mut inbound, disposition)) = self
            .ingress
            .try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
                FairV2IngressBarrierBypass::TimeoutVoteEpisode,
                |inbound| {
                    let BlockMessage::V2(message) = inbound.message() else {
                        return false;
                    };
                    inbound.ingress_ownership().is_some_and(|ownership| {
                        executor.can_admit_timeout_vote_recovery_episode(message, ownership)
                    })
                },
            )?
        else {
            self.assert_missing_proposal_serve_selected();
            return Ok(());
        };
        if disposition != super::super::FairV2IngressDequeueDisposition::Admit {
            return Err("timeout episode selected an obsolete leader-wire owner".to_owned());
        }
        let mut ownership = inbound
            .take_ingress_ownership()
            .ok_or_else(|| "selected TimeoutVote lost fair-ingress ownership".to_owned())?;
        self.ingress
            .bind_leader_wire_runtime_ownership(&mut ownership)?;
        let (message, _, _) = inbound.into_message_sender_and_reply_routes();
        let BlockMessage::V2(message) = message else {
            return Err("timeout episode selected a non-v2 message".to_owned());
        };
        self.executor
            .enqueue_network_with_ingress_ownership(message, ownership)
            .map_err(|error| error.to_string())?;
        self.remote_timeout_votes_admitted = self.remote_timeout_votes_admitted.saturating_add(1);
        self.assert_missing_proposal_serve_selected();
        Ok(())
    }
    /// Execute and deliver the local timeout signature through the worker completion lane.
    pub(in crate::sumeragi) fn service_timeout_recovery_prefix(&mut self) -> Result<(), String> {
        match self.command_rx.try_recv() {
            Ok(V2IoCommand::Sign {
                task,
                restore_outbound_payload: false,
            }) if matches!(task.request(), SignRequest::TimeoutVote(_)) => {
                let work_id = task.id();
                let lifecycle_ordinal = task.lifecycle_ordinal();
                let signature = Signature::new(
                    self.local_key.private_key(),
                    &task.request().signature_preimage(),
                )
                .payload()
                .to_vec();
                self.command_rx.complete_work(work_id);
                try_send_tracked_completion_with_lifecycle_ordinal(
                    &self.completion_tx,
                    &self.completion_admission,
                    V2IoCompletion::Signature {
                        work_id,
                        signature,
                        outbound_payload: None,
                    },
                    Some(lifecycle_ordinal),
                )
                .map_err(|_| {
                    "selected-Serve timeout completion channel is unavailable".to_owned()
                })?;
                self.local_timeout_signature_completed = true;
            }
            Ok(_) => {
                return Err(
                    "selected-Serve timeout fixture received an unexpected I/O command".to_owned(),
                );
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("selected-Serve timeout worker disconnected".to_owned());
            }
        }
        if let Some(cut) = self
            .executor
            .timeout_recovery_lifecycle_cut()
            .map_err(|error| error.to_string())?
        {
            self.timeout_prefix_completions = self.timeout_prefix_completions.saturating_add(
                self.services
                    .drain_timeout_recovery_prefix_completion(&mut self.executor, cut)
                    .map_err(|error| error.to_string())?,
            );
        }
        self.assert_missing_proposal_serve_selected();
        Ok(())
    }
    /// Run one typed pacemaker transition while the exact Serve carrier remains selected.
    pub(in crate::sumeragi) fn service_pacemaker(&mut self) -> Result<(), String> {
        self.executor
            .set_ingress_physical_cut(self.ingress.next_physical_admission_ordinal())
            .map_err(|error| error.to_string())?;
        let _ = self
            .executor
            .step_pacemaker_once(Instant::now(), &mut self.services)
            .map_err(|error| error.to_string())?;
        self.assert_missing_proposal_serve_selected();
        Ok(())
    }
    /// Return whether the real reducer and production service both installed view one.
    pub(in crate::sumeragi) fn entered_view_one(&self) -> bool {
        self.executor.current_tag().view() == 1 && self.services.active_tag.view() == 1
    }
    /// Check the complete local + dual-remote timeout recovery result.
    pub(in crate::sumeragi) fn assert_complete(&self) {
        self.assert_missing_proposal_serve_selected();
        assert!(self.local_timeout_signature_completed);
        assert_eq!(self.remote_timeout_votes_admitted, 2);
        assert_eq!(self.timeout_prefix_completions, 1);
        assert_eq!(self.ingress.len(), 1, "only the missing-body Serve remains");
        assert!(self.entered_view_one());
        let observations = self
            .consensus_observations
            .lock()
            .expect("inspect selected-Serve consensus broadcasts");
        assert!(observations.iter().any(|(_, message)| matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                if vote.signer
                    == self.services.local_validator.expect("fixture is a validator")
        )));
        assert!(observations.iter().any(|(_, message)| matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate)
                if certificate.round.view == 0
                    && certificate
                        .groups
                        .iter()
                        .map(|group| group.signers.len())
                        .sum::<usize>()
                        == 3
        )));
    }
    fn assert_missing_proposal_serve_selected(&self) {
        let barrier = self
            .serve_gate
            .selected_barrier()
            .expect("inspect missing-proposal Serve barrier")
            .expect("missing-proposal Serve remains selected");
        assert_eq!(barrier.request_hash(), self.missing_proposal_request_hash);
    }
}
#[cfg(feature = "bls")]
impl Drop for SelectedServeTimeoutRecoveryFixture {
    fn drop(&mut self) {
        // This fixture drives the worker endpoints synchronously and has
        // no background thread to acknowledge a queued Shutdown command.
        drop(self.services.io.take());
    }
}
