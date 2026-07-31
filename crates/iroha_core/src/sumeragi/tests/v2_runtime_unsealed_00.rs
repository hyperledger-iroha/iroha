    use std::collections::VecDeque;

    use crate::sumeragi::v2_core::Generation;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::peer::PeerId;
    use iroha_p2p::network::{
        NetworkReplyRoute, NetworkReplyRouteError, NetworkReplyRouteTestFixture,
    };
    use tempfile::TempDir;

    use super::*;
    use crate::sumeragi::{
        InboundBlockMessage,
        message::BlockMessage,
        v2::{
            AdapterFingerprints, DeferredBodyPipelineStageForTest, SignRequest,
            VerifiedHeightContext,
        },
    };

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
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new([marker, 7]),
            Hash::new([marker, 8]),
            Hash::new([marker, 9]),
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

    #[test]
    fn real_adapter_signature_completion_precedes_deferred_timeout_and_newer_ingress() {
        let directory = TempDir::new().expect("temporary real-adapter ordering directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let start = Instant::now();
        runtime
            .arm_live_clocks(start)
            .expect("arm runtime after adapter startup");

        // Refresh the derived clock before the signer becomes busy. This keeps
        // the absolute deadline and retransmission deadline independent in the
        // ordering trace below.
        let before_timeout = start + Duration::from_secs(9);
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("service pre-fence retransmission"),
            RuntimeStep::Advanced(_)
        ));

        let proposal = signed_runtime_proposal(&context, &keys, 0xE1);
        runtime
            .enqueue_network(proposal.clone())
            .expect("enqueue authenticated proposal");
        let proposal_effects = match runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch authenticated proposal")
        {
            RuntimeStep::Advanced(effects) => effects,
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
        let (tag, manifest) = match proposal_effects.as_slice() {
            [
                AdapterEffect::FetchBody {
                    tag,
                    manifest: Some(manifest),
                    ..
                },
            ] => (*tag, manifest.clone()),
            effects => panic!("unexpected proposal effects: {effects:?}"),
        };

        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("enqueue reconstructed body");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("dispatch reconstructed body"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
        ));
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("enqueue durable-body completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("dispatch durable-body completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));
        runtime
            .enqueue_validation_succeeded(
                tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            )
            .expect("enqueue validated-body completion");
        let (prepare_sign_tag, prepare_signature_preimage) = match runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch validated-body completion")
        {
            RuntimeStep::Advanced(effects) => match effects.as_slice() {
                [
                    AdapterEffect::Sign {
                        tag,
                        request: SignRequest::Vote(vote),
                    },
                ] if vote.phase == wire::GlobalPhase::Prepare
                    && vote.round == manifest.round
                    && vote.subject == manifest.subject =>
                {
                    (*tag, vote.signature_preimage())
                }
                effects => panic!("unexpected validation effects: {effects:?}"),
            },
            RuntimeStep::Idle => panic!("validation dispatch unexpectedly idle"),
        };

        // The body pipeline leaves the fair-ingress cursor at Progress. An
        // exact authenticated retransmission is consumed below the reducer
        // fence and advances that cursor normally, so Completion owns the
        // first slot once the signature and newer ingress arrive together.
        runtime
            .enqueue_network(proposal)
            .expect("enqueue exact authenticated retransmission");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("coalesce exact authenticated retransmission"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.ingress.next_class, CommandClass::Completion);

        let deadline = start + runtime.round_timeout();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("deliver absolute timeout through the real adapter"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert!(
            !runtime.driver().deferred_work_is_serviceable(),
            "the exact Prepare signature still fences the Busy-deferred timeout"
        );

        let prepare_signature = Signature::new(keys[0].private_key(), &prepare_signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(prepare_sign_tag, prepare_signature)
            .expect("enqueue exact Prepare signature completion");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0xE2))
            .expect("enqueue newer authenticated ingress");
        assert_eq!(runtime.queued_commands(), 2);

        let prepare_broadcast = runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("signature completion owns the first serialized turn");
        assert!(matches!(
            prepare_broadcast,
            RuntimeStep::Advanced(ref effects)
                if matches!(
                    effects.as_slice(),
                    [AdapterEffect::Broadcast(message)]
                        if matches!(
                            &message.payload,
                            wire::ConsensusMessageV2Payload::Vote(vote)
                                if vote.phase == wire::GlobalPhase::Prepare
                                    && vote.round == manifest.round
                                    && vote.subject == manifest.subject
                        )
                )
        ));
        assert_eq!(
            runtime.queued_commands(),
            1,
            "newer ingress remains owned after signature completion"
        );

        let timeout_macro_step = runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("service exactly one older Busy-deferred timeout transition");
        assert!(matches!(
            timeout_macro_step,
            RuntimeStep::Advanced(ref effects)
                if matches!(
                    effects.as_slice(),
                    [AdapterEffect::Sign {
                        request: SignRequest::TimeoutVote(vote),
                        ..
                    }] if vote.round == manifest.round
                )
        ));
        assert_eq!(
            runtime.queued_commands(),
            1,
            "one deferred macro-step cannot concatenate newer ingress"
        );

        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("dispatch newer ingress"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ReportEquivocation { .. }])
        ));
        assert_eq!(runtime.queued_commands(), 0);

        let next_retransmission = before_timeout + runtime.retransmit_interval();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(next_retransmission)
                .expect("make the next periodic scheduling decision"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.retransmit_started_at, next_retransmission);
    }

    #[test]
    fn round_timeout_grows_linearly_by_view_without_wrapping() {
        let base = Duration::from_secs(10);
        assert_eq!(round_timeout_for_view(base, 0), base);
        assert_eq!(round_timeout_for_view(base, 1), Duration::from_secs(20));
        assert_eq!(round_timeout_for_view(base, 7), Duration::from_secs(80));
        assert_eq!(
            round_timeout_for_view(Duration::new(1, 500_000_000), 1),
            Duration::from_secs(3),
        );

        assert_eq!(
            round_timeout_for_view(Duration::from_secs(1), u64::MAX - 1),
            Duration::from_secs(u64::MAX)
        );
        assert_eq!(
            round_timeout_for_view(Duration::from_secs(1), u64::MAX),
            Duration::MAX
        );
        assert_eq!(round_timeout_for_view(Duration::MAX, 1), Duration::MAX);
    }

    #[test]
    fn recovered_nonzero_view_uses_scaled_timeout_from_live_arm() {
        let constructed_at = Instant::now();
        let armed_at = constructed_at + Duration::from_secs(500);
        let recovered = tag(4);
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(recovered),
            constructed_at,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("open recovered runtime");

        runtime
            .arm_live_clocks(armed_at)
            .expect("arm after recovered startup");
        assert_eq!(runtime.round_timeout(), Duration::from_secs(50));
        let _ =
            runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(49));
        assert!(runtime.driver.timeouts.is_empty());
        let _ =
            runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(50));
        assert_eq!(runtime.driver.timeouts, vec![recovered]);
    }

    #[test]
    fn class_aware_ingress_is_bounded_and_reserves_progress_and_completion_slots() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(4, 1, 1),
        );
        assert_eq!(runtime.remaining_completion_capacity(), 4);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .unwrap();
        assert_eq!(runtime.remaining_completion_capacity(), 3);
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(2),
        )
        .unwrap();
        assert_eq!(runtime.remaining_completion_capacity(), 2);
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(99)
            ),
            Err(EnqueueError::ReservedCapacity)
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("reserved progress slot");
        assert_eq!(runtime.remaining_completion_capacity(), 1);
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(4),
        )
        .expect("reserved completion slot");
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        assert_eq!(runtime.queued_commands(), 4);
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Completion,
                FakeCommand::record(5)
            ),
            Err(EnqueueError::Full)
        );

        for offset in 0..4 {
            let _ = runtime
                .step_and_take_scheduler_ownership_for_test(start + Duration::from_millis(offset));
        }
        assert_eq!(
            runtime.driver.delivered,
            vec![(initial, 4), (initial, 3), (initial, 1), (initial, 2)]
        );
    }

    #[test]
    fn scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("normal owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(9),
        )
        .expect("progress owner fits");

        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Advanced(_))));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("FIFO dispatch retains exact scheduler ownership")
            .clone();
        assert_eq!(evidence.selected, RuntimeSelectedOwnerKind::Fifo);
        assert_eq!(evidence.round_tag, owner_tag);
        assert_eq!(evidence.queue_before.len, 2);
        assert_eq!(evidence.queue_after.len, 1);
        assert_eq!(
            evidence.queue_before.service_cursor,
            SERVICE_CLASS_COMPLETION
        );
        assert_eq!(evidence.queue_after.service_cursor, SERVICE_CLASS_NORMAL);
        assert_eq!(evidence.queue_before.max_service_debt, 0);
        assert_eq!(evidence.queue_after.max_service_debt, 1);
        assert!(evidence.live_mode);
        assert!(!evidence.timeout_due);
        assert!(!evidence.periodic_timer_due);
        assert!(evidence.fifo_ready);
        assert!(!evidence.completion_ready);
        assert!(evidence.progress_ready);
        assert!(evidence.normal_ready);
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &evidence.candidate else {
            panic!("FIFO dispatch must carry one exact command candidate");
        };
        assert_eq!(
            candidate.identity,
            FakeCommand::record(9)
                .exact_runtime_command_identity()
                .digest()
        );
        assert_eq!(candidate.kind, RuntimeCommandKind::Test);
        assert_eq!(candidate.class, SERVICE_CLASS_PROGRESS);
        assert_eq!(candidate.tag, owner_tag);
        assert_eq!(candidate.admission_ordinal, 1);
        assert_eq!(candidate.fifo_position, 1);
        assert_eq!(candidate.eligible_skips_before, 0);
        assert_eq!(candidate.eligible_skips_after, 0);
        assert_eq!(evidence.validate_exact(), Ok(()));

        let rejected = |mutated: RuntimeSchedulerOwnershipEvidence| {
            assert_eq!(
                mutated.validate_exact(),
                Err(RuntimeSchedulerEvidenceError::InvalidProjection)
            );
        };

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.identity.canonical_hash = iroha_crypto::Hash::new([0xFF]);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.kind = RuntimeCommandKind::Authenticated;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.class = SERVICE_CLASS_NORMAL;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.tag = tag(99);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.admission_ordinal = 0;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.fifo_position = 0;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_after.service_cursor = SERVICE_CLASS_COMPLETION;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_after.max_service_debt = 0;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.timeout_due = true;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.progress_ready = false;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.fifo_owed_after = true;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence;
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.eligible_skips_before = 1;
        rejected(mutated);
    }

    #[test]
    fn full_lane_retryable_backpressure_restores_and_services_exact_fifo_owner() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        assert!(driver.retry_once.insert(1));
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(3, 1, 1));
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("oldest retryable owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(2),
        )
        .expect("later completion owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("later progress owner fills the lane");
        assert_eq!(runtime.ingress.remaining_capacity(), 0);
        let original = runtime
            .ingress
            .commands
            .front()
            .expect("oldest physical owner is present")
            .clone();

        assert!(matches!(
            runtime.step(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("retry turn retains typed scheduler ownership")
            .clone();
        assert_eq!(
            evidence.selected,
            RuntimeSelectedOwnerKind::FifoRetryRetained
        );
        assert_eq!(evidence.queue_before.len, 3);
        assert_eq!(evidence.queue_after.len, 3);
        assert_eq!(evidence.validate_exact(), Ok(()));
        let restored = runtime
            .ingress
            .commands
            .front()
            .expect("retry restores the original physical owner");
        assert_eq!(restored.tag, original.tag);
        assert_eq!(restored.class, original.class);
        assert_eq!(restored.identity, original.identity);
        assert_eq!(restored.admission_ordinal, original.admission_ordinal);
        assert_eq!(restored.lifecycle_ordinal, original.lifecycle_ordinal);
        assert_eq!(restored.causal_origin, original.causal_origin);
        assert_eq!(runtime.driver.delivered, Vec::new());

        let mut weakened = evidence.clone();
        weakened.selected = RuntimeSelectedOwnerKind::Fifo;
        weakened.projection_hash = runtime_scheduler_projection_hash(&weakened);
        assert_eq!(
            weakened.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "an equal-length retry cannot be relabelled as completed FIFO service"
        );
        assert!(runtime.take_last_scheduler_ownership().is_some());
        assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 1)]);
        assert_eq!(runtime.ingress.len(), 2);
        assert_eq!(
            runtime
                .ingress
                .commands
                .front()
                .and_then(|queued| queued.command.record),
            Some(2),
            "later Completion work cannot overtake the retained lifecycle"
        );
    }

    #[test]
    fn retryable_backpressure_restores_the_exact_recovery_fifo_owner_once() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        assert!(driver.retry_once.insert(7));
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            driver,
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(4, 1, 1),
            Vec::new(),
        )
        .expect("construct unarmed recovery runtime");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("recovery owner fits");
        let original_owner = runtime
            .ingress
            .commands
            .front()
            .expect("recovery owner is present")
            .lifecycle_owner()
            .expect("recovery owner is exact");

        assert!(matches!(
            runtime.step_recovery(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("retrying recovery retains scheduler ownership");
        assert_eq!(
            evidence.selected,
            RuntimeSelectedOwnerKind::RecoveryFifoRetryRetained
        );
        assert_eq!(evidence.queue_before.len, evidence.queue_after.len);
        assert_eq!(evidence.validate_exact(), Ok(()));
        assert_eq!(
            runtime
                .ingress
                .commands
                .front()
                .expect("recovery retry remains physically admitted")
                .lifecycle_owner()
                .expect("restored recovery owner is exact"),
            original_owner
        );
        assert!(runtime.take_last_scheduler_ownership().is_some());
        assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));

        assert!(matches!(
            runtime.step_recovery_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 7)]);
        assert_eq!(runtime.queued_commands(), 0);
    }

    #[test]
    fn adapter_command_identity_is_derived_from_exact_immutable_payload() {
        let owner_tag = tag(4);
        let command = AdapterCommand::SignatureCompleted(vec![0x11, 0x22, 0x33]);
        let expected = command.exact_runtime_command_identity();
        let shared = expected.clone();
        assert!(Arc::ptr_eq(
            &expected.canonical_bytes,
            &shared.canonical_bytes
        ));
        assert_ne!(
            expected,
            AdapterCommand::SignatureCompleted(vec![0x11, 0x22, 0x34])
                .exact_runtime_command_identity()
        );

        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));
        ingress
            .enqueue(TaggedCommand::new(
                owner_tag,
                CommandClass::Completion,
                command,
                Instant::now(),
            ))
            .expect("exact adapter command fits completion capacity");
        let (_, candidate) = ingress
            .pop_next_with_ownership()
            .expect("adapter command retains its admission ordinal")
            .expect("adapter command owns the selected FIFO occurrence");
        assert_eq!(candidate.identity, expected.digest());
        assert_eq!(candidate.kind, RuntimeCommandKind::SignatureCompleted);
        assert_eq!(candidate.class, SERVICE_CLASS_COMPLETION);
        assert_eq!(candidate.tag, owner_tag);
        assert_eq!(candidate.admission_ordinal, 1);
        assert_eq!(candidate.fifo_position, 0);
    }

    #[test]
    fn scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches() {
        let start = Instant::now();
        let owner_tag = tag(0);

        let mut idle = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        assert!(matches!(idle.step(start), Ok(RuntimeStep::Idle)));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::Idle)
        );
        assert!(idle.take_last_scheduler_ownership().is_some());

        assert!(matches!(
            idle.step(start + Duration::from_secs(2)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::PeriodicTimer)
        );
        assert!(idle.take_last_scheduler_ownership().is_some());
        assert!(matches!(
            idle.step(start + Duration::from_secs(10)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::Timeout)
        );

        let (mut recovery, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(owner_tag),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(6, 2, 1),
            Vec::new(),
        )
        .expect("construct unarmed recovery runtime");
        enqueue_fake(
            &mut recovery,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("recovery FIFO owner fits");
        assert!(matches!(
            recovery.step_recovery(start),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::RecoveryFifo)
        );
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .expect("recovery FIFO retains evidence")
                .validate_exact(),
            Ok(())
        );
        assert!(
            !recovery
                .last_scheduler_ownership()
                .expect("recovery FIFO retains evidence")
                .live_mode
        );
        assert!(recovery.take_last_scheduler_ownership().is_some());
        assert!(matches!(
            recovery.step_recovery(start),
            Ok(RuntimeStep::Idle)
        ));
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::RecoveryIdle)
        );
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .expect("recovery idle retains evidence")
                .validate_exact(),
            Ok(())
        );

        let mut deferred_driver = FakeDriver::new(owner_tag);
        deferred_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut deferred = runtime(deferred_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(deferred.step(start), Ok(RuntimeStep::Advanced(_))));
        let evidence = deferred
            .last_scheduler_ownership()
            .expect("deferred dispatch retains its typed occurrence");
        assert_eq!(evidence.selected, RuntimeSelectedOwnerKind::Deferred);
        assert_eq!(evidence.validate_exact(), Ok(()));
        assert!(matches!(
            &evidence.candidate,
            RuntimeSelectedCandidateOwnership::ExactDeferred(candidate)
                if candidate.service.admission_ordinal == 0
                    && candidate.service.validate_exact()
                    && candidate.ingress_ownership.is_none()
        ));

        let mut unavailable_driver = FakeDriver::new(owner_tag);
        unavailable_driver.deferred_identity_unavailable = true;
        unavailable_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut unavailable = runtime(unavailable_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(
            unavailable.step(start),
            Err(RuntimeError::FailClosed)
        ));
        assert!(unavailable.last_scheduler_ownership().is_none());
    }

    #[test]
    fn runtime_rejects_replayed_foreign_and_mutated_deferred_tokens() {
        let start = Instant::now();
        let owner_tag = tag(0);

        let mut replay_driver = FakeDriver::new(owner_tag);
        replay_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        replay_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let replayed = DeferredServiceEvidence::completion_for_test(
            &replay_driver.deferred_admission_ordinals,
            owner_tag,
            2,
            DeferredPriority::Completion,
        );
        assert!(replayed.claim_adapter_service_for_test());
        replay_driver
            .deferred_evidence_overrides
            .push_back(replayed.clone());
        replay_driver
            .deferred_evidence_overrides
            .push_back(replayed);
        let mut replay = runtime(replay_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(replay.step(start), Ok(RuntimeStep::Advanced(_))));
        assert!(replay.take_last_scheduler_ownership().is_some());
        assert!(matches!(replay.step(start), Err(RuntimeError::FailClosed)));

        let mut foreign_driver = FakeDriver::new(owner_tag);
        foreign_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let foreign_source = DeferredAdmissionOrdinalSource::new(0);
        let foreign_evidence = DeferredServiceEvidence::completion_for_test(
            &foreign_source,
            owner_tag,
            1,
            DeferredPriority::Completion,
        );
        assert!(foreign_evidence.claim_adapter_service_for_test());
        foreign_driver
            .deferred_evidence_overrides
            .push_back(foreign_evidence);
        let mut foreign = runtime(foreign_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(foreign.step(start), Err(RuntimeError::FailClosed)));

        let mut mutated_driver = FakeDriver::new(owner_tag);
        mutated_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut mutated = DeferredServiceEvidence::completion_for_test(
            &mutated_driver.deferred_admission_ordinals,
            owner_tag,
            1,
            DeferredPriority::Completion,
        );
        assert!(mutated.claim_adapter_service_for_test());
        mutated.protected_progress = true;
        mutated_driver
            .deferred_evidence_overrides
            .push_back(mutated);
        let mut mutated = runtime(mutated_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(mutated.step(start), Err(RuntimeError::FailClosed)));
    }

    #[test]
    fn scheduler_owner_must_be_taken_before_a_later_step_can_enter() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut blocked_runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );

        assert!(matches!(blocked_runtime.step(start), Ok(RuntimeStep::Idle)));
        let first_projection_hash = blocked_runtime
            .last_scheduler_ownership()
            .expect("first idle selection retains a carrier")
            .projection_hash;

        let periodic_at = start + blocked_runtime.retransmit_interval();
        assert!(matches!(
            blocked_runtime.step(periodic_at),
            Err(RuntimeError::FailClosed)
        ));
        assert_eq!(
            blocked_runtime.fail_closed_reason.as_deref(),
            Some("live scheduling began with an unconsumed scheduler owner")
        );
        blocked_runtime.latch_fail_closed("a later generic failure");
        assert_eq!(
            blocked_runtime.fail_closed_reason.as_deref(),
            Some("live scheduling began with an unconsumed scheduler owner"),
            "fail-closed diagnostics retain the first invariant violation"
        );
        let retained = blocked_runtime
            .last_scheduler_ownership()
            .expect("failed re-entry preserves the first unconsumed carrier");
        assert_eq!(retained.selected, RuntimeSelectedOwnerKind::Idle);
        assert_eq!(retained.projection_hash, first_projection_hash);

        let mut runtime = self::runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Idle)));

        let taken = runtime
            .take_last_scheduler_ownership()
            .expect("effect boundary takes the exact first occurrence");
        assert_eq!(taken.selected, RuntimeSelectedOwnerKind::Idle);
        assert_eq!(taken.validate_exact(), Ok(()));
        assert!(runtime.last_scheduler_ownership().is_none());

        assert!(matches!(
            runtime.step(periodic_at),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::PeriodicTimer)
        );
        assert!(runtime.last_scheduler_ownership().is_none());
    }

    #[test]
    fn admission_ordinal_exhaustion_fails_runtime_closed() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        runtime.ingress.lifecycle_ordinals =
            RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 2);
        runtime.ingress.next_admission_ordinal = Some(u128::MAX - 1);
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("the last ordinal with a representable successor is valid");
        assert_eq!(
            runtime.ingress.commands[0].admission_ordinal,
            Some(u128::MAX - 1)
        );
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(2),
            ),
            Err(EnqueueError::FailClosed)
        );
        assert!(runtime.fail_closed);
    }

    #[test]
    fn selected_owner_without_a_runtime_minted_ordinal_fails_closed() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        runtime.ingress.commands.push_back(TaggedCommand::new(
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
            start,
        ));

        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert!(runtime.fail_closed);
        assert!(runtime.last_scheduler_ownership().is_none());
    }

    #[test]
    fn corrupt_cached_identity_and_rebound_origin_are_rejected_before_service() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
        let mut corrupt = TaggedCommand::new(
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
            admitted_at,
        );
        corrupt.identity.canonical_hash = iroha_crypto::Hash::new(b"corrupt cached identity");
        assert_eq!(ingress.enqueue(corrupt), Err(EnqueueError::FailClosed));
        assert!(ingress.commands.is_empty());

        let root = FakeCommand::record(2);
        let mut origin =
            RuntimeCandidateCausalOrigin::mint(owner_tag, CommandClass::Normal, &root, None);
        assert!(origin.bind_lifecycle_ordinal(7));
        assert!(matches!(
            TaggedCommand::with_causal_origin(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(3),
                admitted_at,
                origin,
                8,
            ),
            Err(EnqueueError::FailClosed)
        ));
    }

    #[test]
    fn lifecycle_owner_constructor_rejects_a_conflicting_prebound_ordinal() {
        let owner_tag = tag(0);
        let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"prebound-owner",
        );
        assert!(origin.bind_lifecycle_ordinal(7));
        assert!(matches!(
            RuntimeLifecycleOwner::new(origin.clone(), 8),
            Err(EnqueueError::FailClosed)
        ));
        let exact = RuntimeLifecycleOwner::new(origin, 7)
            .expect("the already-bound exact ordinal remains admissible");
        assert!(exact.validate_exact());
        assert_eq!(exact.lifecycle_ordinal(), 7);
    }

    #[test]
    fn global_lifecycle_minimum_blocks_later_fifo_until_its_completion_arrives() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        let older = runtime
            .mint_fresh_lifecycle_owner(
                owner_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"older external exact request",
            )
            .expect("mint the older externally retained lifecycle");
        runtime
            .configure_external_lifecycle_owner_capacity(4)
            .expect("install the independent asynchronous bound");
        runtime
            .set_external_lifecycle_owners(vec![older.clone()])
            .expect("publish the older external owner");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(9),
        )
        .expect("enqueue later unrelated work");

        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Idle)));
        let idle = runtime
            .take_last_scheduler_ownership()
            .expect("blocked scheduling still publishes exact Idle evidence");
        assert_eq!(idle.selected, RuntimeSelectedOwnerKind::Idle);
        assert!(!idle.fifo_ready);
        assert_eq!(runtime.queued_commands(), 1);

        let due = start + Duration::from_secs(10);
        assert!(matches!(runtime.step(due), Ok(RuntimeStep::Idle)));
        runtime
            .take_last_scheduler_ownership()
            .expect("blocked due clocks publish exact Idle evidence");
        assert!(runtime.timeout_owner.is_some());
        assert!(runtime.retransmit_owner.is_some());
        assert!(runtime.driver.timeouts.is_empty());
        assert!(runtime.driver.retransmits.is_empty());

        let older_effect = RuntimeEffectOwnership::fresh(
            older.clone(),
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
        );
        runtime
            .enqueue_with_lifecycle_owner(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                &older_effect,
            )
            .expect("enqueue the exact older completion");
        assert!(matches!(
            runtime.step(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("completion selection publishes exact ownership");
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = selected.candidate else {
            panic!("older completion must be the exact FIFO candidate");
        };
        assert_eq!(candidate.fifo_position, 1);
        assert_eq!(candidate.lifecycle_ordinal, older.lifecycle_ordinal());
        runtime
            .take_effect_ownership(1)
            .expect("test executor consumes the completion effect owner");
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 1)]);
        assert_eq!(runtime.queued_commands(), 1);

        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("the asynchronous owner retires after its exact completion handoff");
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the older queued FIFO command now drains");
        assert_eq!(
            runtime.driver.delivered,
            vec![(owner_tag, 1), (owner_tag, 9)]
        );
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the frozen timeout drains after all older lifecycles");
        assert_eq!(runtime.driver.timeouts, vec![owner_tag]);
        assert!(runtime.timeout_owner.is_none());
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the later frozen retransmission drains next");
        assert_eq!(runtime.driver.retransmits, vec![owner_tag]);
        assert!(runtime.retransmit_owner.is_none());
    }

    #[test]
    fn external_owner_bound_uses_effect_capacity_not_small_ingress_capacity() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        let pending_bound = 1_024usize;
        runtime
            .configure_external_lifecycle_owner_capacity(pending_bound)
            .expect("configure the executor's independent pending-work bound");
        let exact_capacity = pending_bound + MAX_EFFECTS_PER_STEP;
        let owners = (0..exact_capacity)
            .map(|ordinal| {
                let ordinal = u128::try_from(ordinal).expect("small test owner ordinal");
                let semantic = ordinal.to_le_bytes();
                RuntimeLifecycleOwner::new(
                    RuntimeCandidateCausalOrigin::mint_fresh_root(
                        owner_tag,
                        CommandClass::Progress,
                        RuntimeFreshRootKind::HistoricalLockedRetransmit,
                        &semantic,
                    ),
                    ordinal,
                )
                .expect("synthetic external owner binds its first ordinal")
            })
            .collect::<Vec<_>>();
        runtime
            .set_external_lifecycle_owners(owners)
            .expect("1024 pending owners plus one retained batch fit despite ingress capacity 8");
        assert_eq!(runtime.external_lifecycle_owners.len(), exact_capacity);
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn restart_and_periodic_historical_retries_reuse_one_lifecycle_owner() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let historical = FakeEffect::historical(0xA5);
        let (mut runtime, startup) = SerializedV2Runtime::with_driver(
            FakeDriver::new(owner_tag),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            vec![historical],
        )
        .expect("construct deterministic restart ownership");
        assert_eq!(startup, vec![historical]);
        let startup_owner = runtime
            .take_effect_ownership(1)
            .expect("consume startup ownership")
            .pop()
            .expect("one startup owner");
        assert_eq!(
            startup_owner.causality(),
            RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::StartupRecovery)
        );
        runtime
            .arm_live_clocks(start)
            .expect("startup dispatch completes before clocks arm");
        runtime.driver.timer_effects.push_back(vec![historical]);
        runtime.driver.timer_effects.push_back(vec![historical]);

        let mut retry_owners = Vec::new();
        for elapsed in [2, 4] {
            let RuntimeStep::Advanced(effects) = runtime
                .step(start + Duration::from_secs(elapsed))
                .expect("periodic historical retry dispatches")
            else {
                panic!("periodic historical retry must advance");
            };
            assert_eq!(effects, vec![historical]);
            runtime
                .take_last_scheduler_ownership()
                .expect("periodic retry publishes scheduler ownership");
            retry_owners.push(
                runtime
                    .take_effect_ownership(1)
                    .expect("consume retry ownership")
                    .pop()
                    .expect("one retry owner"),
            );
        }
        assert!(retry_owners.iter().all(|ownership| {
            ownership.causality()
                == RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::HistoricalLockedRetransmit)
                && ownership.owner() == startup_owner.owner()
        }));
        let cache_after_owned_retries = runtime.dormant_fresh_lifecycle_owners.len();
        assert_ne!(cache_after_owned_retries, 0);
        for elapsed in [6, 8] {
            let RuntimeStep::Advanced(effects) = runtime
                .step(start + Duration::from_secs(elapsed))
                .expect("drained historical lifecycle still services its periodic clock")
            else {
                panic!("the periodic clock must advance even after exact work drains")
            };
            assert!(
                effects.is_empty(),
                "a drained exact historical request cannot recreate physical work"
            );
            runtime
                .take_last_scheduler_ownership()
                .expect("proofless periodic stutter retains scheduler ownership");
            assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
            assert_eq!(runtime.queued_commands(), 0);
            assert_eq!(
                runtime.dormant_fresh_lifecycle_owners.len(),
                cache_after_owned_retries,
                "proofless retransmission cannot mint a replacement dormant owner"
            );
        }
        assert_eq!(runtime.driver.retransmits, vec![owner_tag; 4]);

        let next_tag = tag(1);
        runtime
            .observe_effects_with_test_ownership(
                start + Duration::from_secs(9),
                &[FakeEffect::enter_view(next_tag)],
            )
            .expect("test EnterView retains positional producer ownership");
        assert!(
            runtime.dormant_fresh_lifecycle_owners.is_empty(),
            "certified view transition purges every prior-view dormant alias"
        );
    }

    #[test]
    fn dormant_fresh_owner_cache_is_derived_bounded_and_purged_by_view() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let queue = RuntimeQueueConfig::new(8, 2, 2);
        let exact_capacity = queue.capacity + MAX_EFFECTS_PER_STEP;
        let mut runtime = runtime(FakeDriver::new(owner_tag), start, queue);
        let mut last_ordinal = None;
        for identity in 0..exact_capacity {
            let identity = u128::try_from(identity)
                .expect("small dormant-cache fixture")
                .to_le_bytes();
            let owner = runtime
                .mint_fresh_lifecycle_owner(
                    owner_tag,
                    CommandClass::Progress,
                    RuntimeFreshRootKind::HistoricalLockedRetransmit,
                    &identity,
                )
                .expect("derived dormant-cache capacity admits every configured owner");
            last_ordinal = Some(owner.lifecycle_ordinal());
        }
        assert_eq!(runtime.dormant_fresh_lifecycle_owners.len(), exact_capacity);
        assert_eq!(
            runtime.mint_fresh_lifecycle_owner(
                owner_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"one owner beyond the derived bound",
            ),
            Err(EnqueueError::Full)
        );

        let next_tag = tag(1);
        runtime
            .observe_effects_with_test_ownership(start, &[FakeEffect::enter_view(next_tag)])
            .expect("test EnterView retains positional producer ownership");
        assert!(runtime.dormant_fresh_lifecycle_owners.is_empty());
        let successor = runtime
            .mint_fresh_lifecycle_owner(
                next_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"successor-view exact request",
            )
            .expect("view reclamation reopens the same derived cache geometry");
        assert!(
            successor.lifecycle_ordinal() > last_ordinal.expect("cache was filled"),
            "cache reclamation cannot reuse an old admission ordinal"
        );
    }

    #[test]
    fn causal_successors_inherit_root_and_lifecycle_ordinal() {
        let admitted_at = Instant::now();
        let root_tag = tag(0);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 2, 2));
        ingress
            .enqueue(TaggedCommand::new(
                root_tag,
                CommandClass::Normal,
                FakeCommand::record(1),
                admitted_at,
            ))
            .expect("root candidate is admitted");
        let (root, root_owner) = ingress
            .pop_next_with_ownership()
            .expect("root selection is exact")
            .expect("root candidate is ready");
        assert_eq!(root.lifecycle_ordinal, Some(root_owner.lifecycle_ordinal));

        let successor_tag = EventTag::new(
            root_tag.height(),
            root_tag.view() + 1,
            Generation::new(root_tag.generation().get() + 1),
        );
        for value in [2, 3, 4] {
            ingress
                .enqueue(
                    TaggedCommand::with_causal_origin(
                        successor_tag,
                        CommandClass::Completion,
                        FakeCommand::record(value),
                        admitted_at,
                        root_owner.causal_origin.clone(),
                        root_owner.lifecycle_ordinal,
                    )
                    .expect("causal owner is internally consistent"),
                )
                .expect("causal child is admitted with a unique physical owner");
        }

        let physical_ordinals = ingress
            .commands
            .iter()
            .map(|candidate| {
                assert_eq!(
                    candidate.causal_origin, root_owner.causal_origin,
                    "evidence/view rewriting cannot replace the first-admission root"
                );
                assert_eq!(
                    candidate.lifecycle_ordinal,
                    Some(root_owner.lifecycle_ordinal),
                    "every child inherits one logical lifecycle ordinal"
                );
                candidate
                    .admission_ordinal
                    .expect("every physical child has its own FIFO ordinal")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(physical_ordinals.len(), 3);

        let unrelated = TaggedCommand::new(
            successor_tag,
            CommandClass::Completion,
            FakeCommand::record(2),
            admitted_at,
        );
        assert!(
            !unrelated
                .causal_origin
                .same_lifecycle(&root_owner.causal_origin),
            "a physically similar command with a different causal root cannot coalesce"
        );
    }

    #[test]
    fn preassigned_batch_lifecycles_require_shared_mint_and_exact_root() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let unminted_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut unminted_ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            unminted_source.clone(),
        );
        let unminted_command = FakeCommand::record(1);
        let mut unminted_origin = RuntimeCandidateCausalOrigin::mint(
            owner_tag,
            CommandClass::Completion,
            &unminted_command,
            None,
        );
        assert!(unminted_origin.bind_lifecycle_ordinal(1));
        let unminted = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            unminted_command,
            admitted_at,
            unminted_origin,
            1,
        )
        .expect("construct internally exact but unminted lifecycle");
        assert_eq!(
            unminted_ingress.enqueue_completion_batch(vec![unminted]),
            Err(EnqueueError::FailClosed)
        );
        assert!(unminted_ingress.commands.is_empty());
        assert_eq!(
            unminted_source
                .next_ordinal_for_test()
                .expect("unminted batch rejection preserves the source"),
            Some(1)
        );

        let collision_source = RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let mut collision_ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            collision_source.clone(),
        );
        collision_ingress
            .enqueue(TaggedCommand::new(
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(2),
                admitted_at,
            ))
            .expect("mint one exact lifecycle root");
        let (_, root_owner) = collision_ingress
            .pop_next_with_ownership()
            .expect("select the minted root exactly")
            .expect("root is ready");
        let sibling = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(3),
            admitted_at,
            root_owner.causal_origin.clone(),
            root_owner.lifecycle_ordinal,
        )
        .expect("construct one legitimate causal sibling");
        let conflicting_command = FakeCommand::record(4);
        let mut conflicting_origin = RuntimeCandidateCausalOrigin::mint(
            owner_tag,
            CommandClass::Completion,
            &conflicting_command,
            None,
        );
        assert!(conflicting_origin.bind_lifecycle_ordinal(root_owner.lifecycle_ordinal));
        let conflicting = TaggedCommand::with_causal_origin(
            owner_tag,
            CommandClass::Completion,
            conflicting_command,
            admitted_at,
            conflicting_origin,
            root_owner.lifecycle_ordinal,
        )
        .expect("construct a distinct root at the colliding ordinal");
        let next_before_collision = collision_source
            .next_ordinal_for_test()
            .expect("inspect source before batch collision");
        assert_eq!(
            collision_ingress.enqueue_completion_batch(vec![sibling, conflicting]),
            Err(EnqueueError::FailClosed)
        );
        assert!(
            collision_ingress.commands.is_empty(),
            "batch collision must reject atomically"
        );
        assert_eq!(
            collision_source
                .next_ordinal_for_test()
                .expect("batch collision preserves the source"),
            next_before_collision,
            "collision validation must run before reserving physical positions"
        );
    }

