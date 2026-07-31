    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeCommand {
        record: Option<u8>,
        enter_view: Option<EventTag>,
        fail: bool,
    }

    impl FakeCommand {
        const fn record(value: u8) -> Self {
            Self {
                record: Some(value),
                enter_view: None,
                fail: false,
            }
        }

        const fn enter_view(tag: EventTag) -> Self {
            Self {
                record: None,
                enter_view: Some(tag),
                fail: false,
            }
        }

        const fn fail() -> Self {
            Self {
                record: None,
                enter_view: None,
                fail: true,
            }
        }
    }

    impl ExactRuntimeCommandIdentity for FakeCommand {
        fn exact_runtime_command_identity(&self) -> RuntimeCommandIdentity {
            let mut identity = Vec::new();
            match self.record {
                Some(value) => {
                    identity.push(1);
                    identity.push(value);
                }
                None => identity.push(0),
            }
            match self.enter_view {
                Some(tag) => {
                    identity.push(1);
                    append_runtime_identity_tag(&mut identity, tag);
                }
                None => identity.push(0),
            }
            identity.push(u8::from(self.fail));
            let canonical_hash = iroha_crypto::Hash::new(&identity);
            RuntimeCommandIdentity {
                kind: RuntimeCommandKind::Test,
                canonical_bytes: Arc::from(identity),
                canonical_hash,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeEffect {
        enter_view: Option<EventTag>,
        fresh: Option<RuntimeFreshRootKind>,
        semantic: u8,
    }

    impl FakeEffect {
        const fn other() -> Self {
            Self {
                enter_view: None,
                fresh: None,
                semantic: 0,
            }
        }

        const fn enter_view(tag: EventTag) -> Self {
            Self {
                enter_view: Some(tag),
                fresh: None,
                semantic: 0,
            }
        }

        const fn historical(semantic: u8) -> Self {
            Self {
                enter_view: None,
                fresh: Some(RuntimeFreshRootKind::HistoricalLockedRetransmit),
                semantic,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeError;

    impl fmt::Display for FakeError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("fake driver failure")
        }
    }

    impl std::error::Error for FakeError {}

    struct FakeDriver {
        current_tag: EventTag,
        delivered: Vec<(EventTag, u8)>,
        timeouts: Vec<EventTag>,
        retransmits: Vec<EventTag>,
        retry_once: BTreeSet<u8>,
        timer_effects: VecDeque<Vec<FakeEffect>>,
        deferred_effects: VecDeque<Vec<FakeEffect>>,
        deferred_dispatches: usize,
        deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
        deferred_service_cursor: DeferredPriority,
        deferred_identity_unavailable: bool,
        deferred_evidence_overrides: VecDeque<DeferredServiceEvidence>,
        admission_preflight_override: Option<RuntimeCommandAdmissionPreflight>,
        dormant_local_fifo_reservations: Vec<RuntimeDormantLocalFifoReservation>,
        protected_commit: Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    }

    impl FakeDriver {
        fn new(tag: EventTag) -> Self {
            Self {
                current_tag: tag,
                delivered: Vec::new(),
                timeouts: Vec::new(),
                retransmits: Vec::new(),
                retry_once: BTreeSet::new(),
                timer_effects: VecDeque::new(),
                deferred_effects: VecDeque::new(),
                deferred_dispatches: 0,
                deferred_admission_ordinals: DeferredAdmissionOrdinalSource::new(0),
                deferred_service_cursor: DeferredPriority::Completion,
                deferred_identity_unavailable: false,
                deferred_evidence_overrides: VecDeque::new(),
                admission_preflight_override: None,
                dormant_local_fifo_reservations: Vec::new(),
                protected_commit: None,
            }
        }
    }

    impl RuntimeDriver for FakeDriver {
        type Command = FakeCommand;
        type Effect = FakeEffect;
        type Error = FakeError;

        fn current_tag(&self) -> EventTag {
            self.current_tag
        }

        fn preflight_command_admission(
            &self,
            _tag: EventTag,
            _command: &Self::Command,
        ) -> RuntimeCommandAdmissionPreflight {
            self.admission_preflight_override
                .unwrap_or(RuntimeCommandAdmissionPreflight::Admit)
        }

        fn dormant_local_fifo_reservations(
            &self,
        ) -> Result<Vec<RuntimeDormantLocalFifoReservation>, String> {
            Ok(self.dormant_local_fifo_reservations.clone())
        }

        fn dispatch(
            &mut self,
            tagged: TaggedCommand<Self::Command>,
        ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
            if tagged.command.fail {
                return Err(FakeError);
            }
            if let Some(tag) = tagged.command.enter_view {
                self.current_tag = tag;
                return Ok(RuntimeDriverDispatch::completed(vec![
                    FakeEffect::enter_view(tag),
                ]));
            }
            let value = tagged.command.record.expect("well-formed fake command");
            if self.retry_once.remove(&value) {
                return Ok(RuntimeDriverDispatch {
                    effects: Vec::new(),
                    deferred_ingress: None,
                    deferred_ordinal: None,
                    retry_unadmitted: true,
                    producer_handoff: None,
                });
            }
            self.delivered.push((tagged.tag, value));
            Ok(RuntimeDriverDispatch::completed(vec![FakeEffect::other()]))
        }

        fn timeout_elapsed(
            &mut self,
            tag: EventTag,
        ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
            self.timeouts.push(tag);
            Ok(RuntimeDriverDispatch::completed(
                self.timer_effects.pop_front().unwrap_or_default(),
            ))
        }

        fn retransmit_elapsed(
            &mut self,
            tag: EventTag,
        ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
            self.retransmits.push(tag);
            Ok(RuntimeDriverDispatch::completed(
                self.timer_effects.pop_front().unwrap_or_default(),
            ))
        }

        fn deferred_work_is_serviceable(&self) -> bool {
            !self.deferred_effects.is_empty()
        }

        fn deferred_admission_ordinal_source(&self) -> &DeferredAdmissionOrdinalSource {
            &self.deferred_admission_ordinals
        }

        fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
            BTreeSet::new()
        }

        fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
            BTreeSet::new()
        }

        fn synthetic_deferred_lifecycle_owner(
            &self,
            evidence: &DeferredServiceEvidence,
        ) -> Option<RuntimeLifecycleOwner> {
            let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
                evidence.original_tag,
                CommandClass::Completion,
                RuntimeFreshRootKind::StartupRecovery,
                b"fake-deferred-owner",
            );
            RuntimeLifecycleOwner::new(origin, evidence.admission_ordinal).ok()
        }

        fn dispatch_deferred(
            &mut self,
            _eligible: &BTreeSet<u128>,
        ) -> Result<
            Option<(
                Vec<Self::Effect>,
                DeferredServiceEvidence,
                Option<ProducerContinuationHandoffToken>,
            )>,
            Self::Error,
        > {
            self.deferred_dispatches = self.deferred_dispatches.saturating_add(1);
            let before = u64::try_from(self.deferred_effects.len())
                .expect("bounded fake deferred queue length fits u64");
            let effects = self.deferred_effects.pop_front().unwrap_or_default();
            if self.deferred_identity_unavailable {
                return Ok(None);
            }
            let evidence = match self.deferred_evidence_overrides.pop_front() {
                Some(evidence) => evidence,
                None => {
                    let evidence = DeferredServiceEvidence::completion_for_test(
                        &self.deferred_admission_ordinals,
                        self.current_tag,
                        before,
                        self.deferred_service_cursor,
                    );
                    assert!(evidence.claim_adapter_service_for_test());
                    evidence
                }
            };
            self.deferred_service_cursor = evidence.service_cursor_after;
            Ok(Some((effects, evidence, None)))
        }

        fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag> {
            effect.enter_view
        }

        fn effect_causality(
            effect: &Self::Effect,
            _source: RuntimeEffectSource,
        ) -> RuntimeEffectCausality {
            effect.fresh.map_or(
                RuntimeEffectCausality::Inherit,
                RuntimeEffectCausality::Fresh,
            )
        }

        fn fresh_effect_semantic_identity(
            effect: &Self::Effect,
            kind: RuntimeFreshRootKind,
        ) -> Vec<u8> {
            vec![kind.code(), effect.semantic]
        }

        fn effect_root_tag(_effect: &Self::Effect) -> Option<EventTag> {
            None
        }

        fn wire_ingress_may_use_progress(&self, payload: &wire::ConsensusMessageV2Payload) -> bool {
            matches!(
                (payload, self.protected_commit),
                (
                    wire::ConsensusMessageV2Payload::Vote(vote),
                    Some((round, subject, execution_commitment))
                ) if vote.phase == wire::GlobalPhase::Commit
                    && vote.round == round
                    && vote.subject == subject
                    && vote.execution_commitment == execution_commitment
            )
        }
    }

    fn tag(view: u64) -> EventTag {
        EventTag::new(7, view, Generation::new(view + 11))
    }

    fn authenticated_proposal_for_test(
        manifest: wire::PayloadManifest,
    ) -> AuthenticatedConsensusMessage {
        AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
                round: manifest.round,
                proposer: 0,
                subject: manifest.subject,
                manifest,
                justification: wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate: None },
                ),
                signature: vec![1],
            }),
        ))
    }

    fn authenticated_runtime_context() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic runtime ingress key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id: "sumeragi-v2-runtime-ingress-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("runtime fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"runtime ingress nexus context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1024 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0x5A; 32],
        };
        (context, keys)
    }

    fn signed_runtime_proposal(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        marker: u8,
    ) -> wire::ConsensusMessageV2 {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
            payload_hash: Hash::new([marker, 2]),
        };
        let body = vec![marker; 4];
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("small runtime fixture body"),
            &[body],
        )
        .expect("valid runtime fixture manifest");
        let proposer = context.leader(round.view);
        let mut proposal = wire::Proposal {
            round,
            proposer,
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
            &proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal))
    }

    fn signed_runtime_vote(
        keys: &[KeyPair],
        round: wire::ConsensusRound,
        phase: wire::GlobalPhase,
        subject: wire::BlockSubject,
        execution_commitment: wire::ExecutionCommitment,
    ) -> wire::ConsensusMessageV2 {
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(vote.signer).expect("small signer index")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote))
    }

    fn fair_runtime_ownership(
        message: &wire::ConsensusMessageV2,
        semantic_origin: PeerId,
        authenticated_via: PeerId,
    ) -> FairV2IngressOwnershipEvidence {
        let mut inbound =
            super::super::fair_v2_ingress_admit_for_test(InboundBlockMessage::from_transport(
                BlockMessage::V2(message.clone()),
                semantic_origin,
                authenticated_via,
            ));
        inbound
            .take_ingress_ownership()
            .expect("real fair ingress attaches exact ownership")
    }

    fn fair_runtime_ownership_at_lifecycle(
        mut ownership: FairV2IngressOwnershipEvidence,
        lifecycle_ordinal: u128,
    ) -> FairV2IngressOwnershipEvidence {
        ownership.first.lifecycle_ordinal = Some(lifecycle_ordinal);
        ownership.latest.lifecycle_ordinal = Some(lifecycle_ordinal);
        assert!(
            ownership.validate_exact(),
            "test lifecycle projection must preserve exact fair ownership"
        );
        ownership
    }

    fn fair_runtime_ownership_with_reply_route(
        message: &wire::ConsensusMessageV2,
        semantic_origin: PeerId,
        authenticated_via: PeerId,
        reply_route: NetworkReplyRoute,
    ) -> FairV2IngressOwnershipEvidence {
        let mut inbound = super::super::fair_v2_ingress_admit_for_test(
            InboundBlockMessage::try_from_transport_with_reply_route(
                BlockMessage::V2(message.clone()),
                semantic_origin,
                authenticated_via,
                reply_route,
            )
            .expect("test transport identities bind the reply capability"),
        );
        inbound
            .take_ingress_ownership()
            .expect("real fair ingress attaches route ownership")
    }

    fn signed_runtime_quorum_certificate(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        marker: u8,
    ) -> wire::QuorumCertificate {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 5])),
            payload_hash: Hash::new([marker, 6]),
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([marker, 7]),
            Hash::new([marker, 8]),
            Hash::new([marker, 9]),
            1,
            Hash::new([marker, 10]),
        );
        let signers = vec![0, 1, 2];
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate runtime fixture certificate"),
        }
    }

    fn signed_runtime_timeout_certificate(
        context: &wire::HeightContext,
        keys: &[KeyPair],
    ) -> wire::TimeoutCertificate {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let signers = vec![0, 1, 2];
        let preimage = wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers,
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                    .expect("aggregate runtime fixture timeout certificate"),
            }],
        }
    }

    fn runtime_manifest(context: &wire::HeightContext, marker: u8) -> wire::PayloadManifest {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 3])),
            payload_hash: Hash::new([marker, 4]),
        };
        let body = vec![marker; 4];
        wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("small runtime manifest body"),
            &[body],
        )
        .expect("valid runtime manifest")
    }

    fn observe_enter_view_for_test(
        runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) {
        assert_eq!(runtime.round_tag(), previous);
        runtime
            .observe_effects_with_test_ownership(
                Instant::now(),
                &[AdapterEffect::EnterView {
                    tag: rebound,
                    certificate: wire::TimeoutCertificate {
                        round: wire::ConsensusRound {
                            view: rebound
                                .view()
                                .checked_sub(1)
                                .expect("test EnterView target has a predecessor"),
                            ..manifest.round
                        },
                        groups: vec![wire::TimeoutVoteGroup {
                            highest_prepare_qc: None,
                            signers: vec![0, 1, 2],
                            aggregate_signature: vec![0xA5; 96],
                        }],
                    },
                    protected_body: Some((manifest.round, manifest.subject)),
                }],
            )
            .expect("test EnterView retains positional producer ownership");
        assert_eq!(runtime.round_tag(), rebound);
    }

    #[test]
    fn body_available_rebind_accepts_same_view_higher_generation() {
        let directory = TempDir::new().expect("temporary same-view rebind directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let initial = runtime.round_tag();
        let view_one = EventTag::new(
            initial.height(),
            1,
            Generation::new(initial.generation().get() + 1),
        );
        let manifest = runtime_manifest(&context, 0x8A);
        observe_enter_view_for_test(&mut runtime, initial, view_one, &manifest);

        stage_completion_for_queue_test(
            &mut runtime,
            view_one,
            AdapterCommand::BodyAvailable {
                manifest: manifest.clone(),
            },
        );
        let causal_origin = runtime.ingress.commands[0].causal_origin.clone();
        let lifecycle_ordinal = runtime.ingress.commands[0].lifecycle_ordinal;
        let rebound = EventTag::new(
            view_one.height(),
            view_one.view(),
            Generation::new(view_one.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut runtime, view_one, rebound, &manifest);

        assert!(
            runtime
                .rebind_body_available(view_one, rebound, &manifest)
                .expect("same-view generation supersession transfers the exact owner")
        );
        assert_eq!(runtime.queued_commands(), 1);
        assert!(matches!(
            runtime.ingress.commands.front(),
            Some(TaggedCommand {
                tag,
                command: AdapterCommand::BodyAvailable {
                    manifest: queued_manifest,
                },
                ..
            }) if *tag == rebound && queued_manifest == &manifest
        ));
        assert_eq!(runtime.ingress.commands[0].causal_origin, causal_origin);
        assert_eq!(
            runtime.ingress.commands[0].lifecycle_ordinal, lifecycle_ordinal,
            "view/generation rebinding retains the logical lifecycle owner"
        );
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner() {
        let directory = TempDir::new().expect("temporary reserved-body rebind directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let initial = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x8B);
        let reservation = runtime
            .reserve_body_available(initial, manifest.clone())
            .expect("reserve an unpublished body completion");
        let source_after_reserve = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect ordinal source after reservation");
        let rebound = EventTag::new(
            initial.height(),
            initial.view() + 1,
            Generation::new(initial.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut runtime, initial, rebound, &manifest);

        assert!(
            runtime
                .rebind_body_available(initial, rebound, &manifest)
                .expect("the unpublished token is a serialized body owner")
        );
        let mut rebound_reservation = reservation;
        rebound_reservation.tag = rebound;
        assert_eq!(
            runtime.ingress.reserved_body_available.as_ref(),
            Some(&rebound_reservation),
        );
        let retry = runtime
            .reserve_body_available(rebound, manifest.clone())
            .expect("rebound exact retry reclaims the immutable root token");
        assert_eq!(retry, rebound_reservation);
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect source after rebound retry"),
            source_after_reserve,
            "rebind and retry cannot remint the token",
        );

        assert!(
            runtime
                .retire_body_available(rebound, &manifest)
                .expect("terminal supersession retires the exact unpublished owner")
        );
        assert!(runtime.ingress.reserved_body_available.is_none());
        assert_eq!(runtime.queued_commands(), 0);
        assert!(!runtime.fail_closed);
    }

    fn authenticated_network_runtime(
        directory: &TempDir,
        queue: RuntimeQueueConfig,
    ) -> (
        SerializedV2Runtime<SumeragiV2Adapter>,
        wire::HeightContext,
        Vec<KeyPair>,
    ) {
        authenticated_network_runtime_with_local_validator(directory, queue, None)
    }

    fn authenticated_network_runtime_with_local_validator(
        directory: &TempDir,
        queue: RuntimeQueueConfig,
        local_validator: Option<wire::ValidatorIndex>,
    ) -> (
        SerializedV2Runtime<SumeragiV2Adapter>,
        wire::HeightContext,
        Vec<KeyPair>,
    ) {
        let (context, keys) = authenticated_runtime_context();
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("runtime fixture proof of possession")
            })
            .collect();
        let verified =
            VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified fixture");
        let (adapter, startup) = SumeragiV2Adapter::open(
            directory.path().join("runtime-ingress-safety.wal"),
            verified,
            local_validator,
            Generation::new(1),
            [0x31; 32],
            AdapterFingerprints {
                node: Hash::new(b"runtime ingress node"),
                build: Hash::new(b"runtime ingress build"),
                config: Hash::new(b"runtime ingress config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open authenticated network runtime adapter");
        assert!(startup.is_empty());
        let runtime = SerializedV2Runtime::new(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            queue,
        )
        .expect("valid authenticated network runtime")
        .0;
        (runtime, context, keys)
    }

    /// Stage an exact completion directly in the bounded FIFO for tests of
    /// queue ownership itself. Production tests use the public enqueue seams,
    /// whose reducer preflight correctly rejects callbacks without a live
    /// phase or exact terminal lifecycle.
    fn stage_completion_for_queue_test(
        runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
        tag: EventTag,
        command: AdapterCommand,
    ) {
        runtime
            .ingress
            .enqueue(TaggedCommand::new(
                tag,
                CommandClass::Completion,
                command,
                Instant::now(),
            ))
            .expect("queue-ownership fixture stages an exact completion");
    }

    fn fair_network_ownership(
        message: &wire::ConsensusMessageV2,
        sender: PeerId,
    ) -> FairV2IngressOwnershipEvidence {
        let mut admitted =
            super::super::fair_v2_ingress_admit_for_test(super::super::InboundBlockMessage::new(
                super::super::message::BlockMessage::V2(message.clone()),
                Some(sender),
            ));
        admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact source ownership")
    }

    struct LeaderWireProposalFixture {
        ingress: Arc<super::super::FairV2Ingress>,
        gate: Arc<super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate>,
        message: wire::ConsensusMessageV2,
        ownership: FairV2IngressOwnershipEvidence,
        receipt: LeaderWireLifecycleRuntimeReceipt,
    }

    fn leader_wire_proposal_fixture(
        directory: &TempDir,
        context: &wire::HeightContext,
        keys: &[KeyPair],
        marker: u8,
        lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    ) -> LeaderWireProposalFixture {
        let message = signed_runtime_proposal(context, keys, marker);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            unreachable!("signed runtime proposal fixture carries Proposal")
        };
        let ingress = Arc::new(super::super::FairV2Ingress::new(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
        ));
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        ingress
            .configure_roster_for_context(roster.clone(), &context.chain_id, context.da_layout)
            .expect("leader-wire runtime fixture geometry");
        ingress.require_leader_wire_lifecycle_gate();
        let capacity =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
                roster.len(),
                context.da_layout.max_chunk_count,
            )
            .expect("finite leader-wire runtime fixture capacity");
        let owner = [marker; 32];
        let recovery_authority =
            super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                context.id(),
                context.height,
                owner,
                proposal.round.view,
                false,
            );
        let (gate, restore) =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
                &directory
                    .path()
                    .join(format!("leader-wire-runtime-{marker}.wal")),
                context.id(),
                context.height,
                owner,
                roster.iter().cloned().collect(),
                capacity,
                context.da_layout.max_chunk_count,
                recovery_authority,
                &[],
                &[],
            )
            .expect("open leader-wire runtime fixture gate");
        ingress
            .bind_leader_wire_lifecycle_gate(
                Arc::clone(&gate),
                restore,
                lifecycle_ordinals,
                context.id(),
                context.height,
            )
            .expect("bind leader-wire runtime fixture gate");
        ingress.open().expect("open leader-wire runtime fixture");
        let semantic_origin = context.roster
            [usize::try_from(proposal.proposer).expect("small fixture proposer")]
        .validator
        .clone();
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(message.clone()),
                Some(semantic_origin),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ));
        let mut admitted = ingress
            .try_recv()
            .expect("drain exact leader-wire proposal fixture");
        let mut ownership = admitted
            .take_ingress_ownership()
            .expect("leader-wire proposal retains fair-ingress ownership");
        ingress
            .bind_leader_wire_runtime_ownership(&mut ownership)
            .expect("bind exact leader-wire runtime receipt");
        let receipt = ownership
            .leader_wire_runtime_receipt()
            .expect("productive proposal carries runtime receipt")
            .clone();
        LeaderWireProposalFixture {
            ingress,
            gate,
            message,
            ownership,
            receipt,
        }
    }

    fn assert_volatile_leader_wire_release(
        fixture: &LeaderWireProposalFixture,
        receipt: &LeaderWireLifecycleRuntimeReceipt,
    ) {
        assert_eq!(receipt, &fixture.receipt);
        fixture
            .ingress
            .mark_leader_wire_volatile_terminal(receipt)
            .expect("publish process-local leader-wire retirement");
        assert_eq!(
            fixture
                .gate
                .earliest_ingress_scheduler_ordinal()
                .expect("read durable leader-wire minimum"),
            None,
            "a retired runtime owner cannot remain an active scheduler predecessor"
        );
        let semantic_origin = fixture.receipt.token().identity.semantic_origin.clone();
        assert!(matches!(
            fixture.ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(fixture.message.clone()),
                Some(semantic_origin),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Coalesced)
        ));
    }

    fn fair_network_ownership_with_route(
        message: &wire::ConsensusMessageV2,
        semantic_origin: PeerId,
        authenticated_via: PeerId,
        route: NetworkReplyRoute,
    ) -> FairV2IngressOwnershipEvidence {
        let inbound = super::super::InboundBlockMessage::try_from_transport_with_reply_route(
            super::super::message::BlockMessage::V2(message.clone()),
            semantic_origin,
            authenticated_via,
            route,
        )
        .expect("test reply route binds the semantic origin and authenticated source");
        let mut admitted = super::super::fair_v2_ingress_admit_for_test(inbound);
        admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact routed ownership")
    }

    fn runtime(
        driver: FakeDriver,
        start: Instant,
        queue: RuntimeQueueConfig,
    ) -> SerializedV2Runtime<FakeDriver> {
        let mut runtime = SerializedV2Runtime::with_driver(
            driver,
            start,
            Duration::from_secs(10),
            queue,
            Vec::new(),
        )
        .expect("valid fake runtime")
        .0;
        runtime
            .arm_live_clocks(start)
            .expect("arm fake runtime after startup");
        runtime
    }

    fn enqueue_fake(
        runtime: &mut SerializedV2Runtime<FakeDriver>,
        tag: EventTag,
        class: CommandClass,
        command: FakeCommand,
    ) -> Result<(), EnqueueError> {
        runtime.enqueue(tag, class, command)
    }

    fn restored_fake_command(
        tag: EventTag,
        class: CommandClass,
        command: FakeCommand,
        causal_lifecycle_key: Hash,
        lifecycle_ordinal: u128,
        producer_stage: u8,
    ) -> TaggedCommand<FakeCommand> {
        let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
            tag,
            class,
            &command,
            None,
            causal_lifecycle_key,
            lifecycle_ordinal,
        )
        .expect("validated dormant metadata reconstructs one exact owner");
        let mut tagged = TaggedCommand::with_causal_origin(
            tag,
            class,
            command,
            Instant::now(),
            owner.causal_origin().clone(),
            owner.lifecycle_ordinal(),
        )
        .expect("restored command binds its persisted ordinal");
        tagged.restored_producer_stage = Some(producer_stage);
        tagged
    }

    #[test]
    fn successor_activation_snapshot_requires_armed_live_clocks() {
        let directory = TempDir::new().expect("temporary successor-clock directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));

        assert!(matches!(
            runtime.successor_activation_status_snapshot(),
            Err(AdapterError::SuccessorClocksNotArmed)
        ));

        runtime
            .arm_live_clocks(Instant::now())
            .expect("arm clocks after all startup work");
        let status = runtime
            .successor_activation_status_snapshot()
            .expect("armed runtime may produce its activation snapshot");
        assert_eq!(status.height_context_id, context.id());
        assert_eq!(status.height, context.height);
        assert!(matches!(
            status.liveness.last_progress,
            Some(wire::SumeragiV2ProgressTransitionStatus {
                transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
                ..
            })
        ));
    }

    #[test]
    fn active_view_producer_fences_timeout_until_exact_proposal_fanout() {
        let (context, keys) = authenticated_runtime_context();
        let message = signed_runtime_proposal(&context, &keys, 0xA7);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
            panic!("runtime fixture must produce a Proposal")
        };
        let initial = EventTag::new(context.height, 0, Generation::new(1));
        let start = Instant::now();
        let (mut runtime, startup) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("construct unarmed active-view producer runtime");
        assert!(startup.is_empty());
        runtime
            .reconcile_active_view_producer(initial, true)
            .expect("reserve the leader producer before clocks arm");
        let reserved = runtime
            .active_view_producer
            .as_ref()
            .expect("leader producer reservation")
            .ownership
            .clone();
        runtime
            .arm_live_clocks(start)
            .expect("arm clocks after producer reservation");

        let ownership = runtime
            .mint_local_proposal_effect_ownership(initial, &proposal.manifest)
            .expect("local Store aliases the active producer");
        assert_eq!(ownership.owner(), reserved.owner());
        assert!(runtime.active_view_producer.is_some());

        let deadline = start + Duration::from_secs(10);
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(deadline),
            Ok(RuntimeStep::Idle)
        ));
        assert!(runtime.driver.timeouts.is_empty());
        assert!(runtime.active_view_producer.is_some());

        runtime
            .complete_active_view_producer_after_proposal_fanout(proposal.round, &ownership)
            .expect("guarded fanout retires the inherited producer");
        assert!(runtime.active_view_producer.is_none());
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(deadline),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        assert_eq!(runtime.driver.timeouts, vec![initial]);
    }

    #[test]
    fn proposal_fanout_cannot_replace_active_view_producer_owner() {
        let (context, keys) = authenticated_runtime_context();
        let message = signed_runtime_proposal(&context, &keys, 0xA8);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
            panic!("runtime fixture must produce a Proposal")
        };
        let initial = EventTag::new(context.height, 0, Generation::new(1));
        let start = Instant::now();
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("construct active-view producer runtime");
        runtime
            .reconcile_active_view_producer(initial, true)
            .expect("reserve exact active producer");
        runtime
            .arm_live_clocks(start)
            .expect("arm after producer reservation");
        let foreign = RuntimeEffectOwnership::fresh_for_test(initial, 999);

        assert!(
            runtime
                .complete_active_view_producer_after_proposal_fanout(proposal.round, &foreign)
                .is_err()
        );
        assert!(runtime.fail_closed);
        assert!(runtime.active_view_producer.is_some());
    }

    #[test]
    fn absolute_timeout_fires_once_and_messages_never_reset_it() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        assert_eq!(runtime.remaining_completion_capacity(), 8);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(10));
        assert_eq!(runtime.retransmit_interval(), Duration::from_secs(2));
        assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(12));

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("enqueue message");
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(1)),
            Ok(RuntimeStep::Advanced(_))
        ));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(runtime.driver.retransmits, vec![initial]);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(2),
        )
        .expect("enqueue second message");
        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9))
            .expect("the older frozen periodic lifecycle dispatches");
        assert_eq!(runtime.driver.retransmits, vec![initial, initial]);
        assert_eq!(runtime.driver.delivered, vec![(initial, 1)]);

        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("the admitted pre-deadline message drains before timeout");
        assert_eq!(runtime.driver.delivered, vec![(initial, 1), (initial, 2)]);
        assert!(runtime.driver.timeouts.is_empty());

        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
            .expect("absolute timeout dispatch succeeds after the finite prefix");
        assert_eq!(runtime.driver.timeouts, vec![initial]);

        runtime
            .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(20))
            .expect("post-timeout scheduling succeeds");
        assert_eq!(
            runtime.driver.retransmits,
            vec![initial, initial, initial],
            "ordinary ingress never resets either clock"
        );
    }

    #[test]
    fn serviceable_adapter_debt_drains_one_macro_step_before_new_work() {
        let start = Instant::now();
        let initial = tag(0);
        let mut driver = FakeDriver::new(initial);
        driver
            .deferred_effects
            .push_back(vec![FakeEffect::other(), FakeEffect::other()]);
        driver.deferred_effects.push_back(vec![FakeEffect::other()]);
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(8, 2, 2));
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .expect("enqueue newer runtime work");

        let due = start + Duration::from_secs(10);
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 2
        ));
        assert_eq!(runtime.driver.deferred_dispatches, 1);
        assert_eq!(runtime.queued_commands(), 1);
        assert!(runtime.driver.timeouts.is_empty());

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.deferred_dispatches, 2);
        assert_eq!(runtime.queued_commands(), 1);
        assert!(runtime.driver.timeouts.is_empty());

        // The finite debt is now empty. The admitted FIFO lifecycle predates
        // the frozen timeout owner and therefore drains first.
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert!(runtime.driver.timeouts.is_empty());
        assert_eq!(runtime.driver.delivered, vec![(initial, 9)]);
        assert_eq!(runtime.queued_commands(), 0);

        // The retained timeout owner then runs without any replenished
        // periodic producer ahead of it.
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        assert_eq!(runtime.driver.timeouts, vec![initial]);
    }

    #[test]
    fn serviceable_adapter_debt_runs_without_runtime_ingress() {
        let start = Instant::now();
        let initial = tag(0);
        let mut driver = FakeDriver::new(initial);
        driver.deferred_effects.push_back(vec![FakeEffect::other()]);
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(8, 2, 2));

        assert_eq!(runtime.queued_commands(), 0);
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.deferred_dispatches, 1);
        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Idle)
        ));
    }
