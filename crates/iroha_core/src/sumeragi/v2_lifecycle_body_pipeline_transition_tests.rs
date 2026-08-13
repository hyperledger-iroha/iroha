#[cfg(all(test, feature = "bls"))]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};

    use super::super::projection;
    use super::*;
    use crate::sumeragi::{
        v2_core::{EventTag, Generation},
        v2_lifecycle_coordinator::replay_authority::{
            CertifiedFetchReplayEvidenceV1, CertifiedStoreReplayEvidenceV1,
            DurableValidateReplayEvidenceV1,
        },
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    };

    pub(super) struct FetchStoreFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        durable_receipt: DurableBodyReceipt,
        store_effect: AdapterEffect,
        store_pending: PendingRuntimeEffectBinding,
        pub(super) store_candidate: CandidateAdmission,
        store_replay: CertifiedStoreReplayEvidenceV1,
    }

    struct StoreValidateFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        durable_receipt: DurableBodyReceipt,
        store_effect: AdapterEffect,
        store_pending: PendingRuntimeEffectBinding,
        validate_effect: AdapterEffect,
        validate_pending: PendingRuntimeEffectBinding,
        validate_candidate: CandidateAdmission,
        validate_replay: DurableValidateReplayEvidenceV1,
    }

    struct ValidateApplyFixture {
        coordinator: LifecycleCoordinator,
        lease: TurnLease,
        verified: VerifiedHeightContext,
        validate_effect: AdapterEffect,
        validate_pending: PendingRuntimeEffectBinding,
        validate_candidate: CandidateAdmission,
        validate_replay: DurableValidateReplayEvidenceV1,
        validated_receipt: ValidatedBodyReceipt,
        apply_effect: AdapterEffect,
        apply_candidate: CandidateAdmission,
    }

    fn capacity_geometry(effect_limit: usize) -> super::super::schema::CapacityGeometry {
        super::super::schema::CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| {
            (
                class,
                if class == CapacityClass::Effect {
                    effect_limit
                } else {
                    1
                },
            )
        }))
    }

    fn lifecycle_context(context: &wire::HeightContext) -> super::super::LifecycleContext {
        let mut id = [0_u8; 32];
        id.copy_from_slice(context.id().0.as_ref());
        super::super::LifecycleContext::new(super::super::LifecycleDigest::new(id), context.height)
    }

    fn verified_context() -> (VerifiedHeightContext, wire::HeightContext) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic Fetch-to-Store BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("Fetch-to-Store proof of possession")
            })
            .collect::<Vec<_>>();
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("fetch-store-transition-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"fetch-store nexus context"),
            execution_policy_hash: Hash::new(b"fetch-store execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 512 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0x51; 32],
        };
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verified Fetch-to-Store height context");
        (verified, context)
    }

    fn fixture_execution_commitment() -> wire::ExecutionCommitment {
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"fetch-store state root"),
            Hash::new(b"fetch-store events root"),
            Hash::new(b"fetch-store trace root"),
            1,
            Hash::new(b"fetch-store fee summary"),
        )
    }

    fn body_manifest(
        verified: &VerifiedHeightContext,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> wire::PayloadManifest {
        wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: verified.context().da_layout,
            chunk_hashes: vec![Hash::new(b"body-pipeline chunk")],
            chunk_root: Hash::new(b"body-pipeline chunk root"),
        }
    }

    fn durable_body_receipt(
        verified: &VerifiedHeightContext,
        manifest: &wire::PayloadManifest,
    ) -> DurableBodyReceipt {
        DurableBodyReceipt::for_test(
            verified.context().id(),
            manifest.round,
            manifest.subject,
            HashOf::new(manifest),
        )
    }

    fn prepare_authorized_body_transition<'a>(
        coordinator: &'a mut LifecycleCoordinator,
        lease: &TurnLease,
        candidate: CandidateAdmission,
        parent_payload: DurablePayloadReference,
        edge: DurableContinuationEdge,
    ) -> Result<PreparedBodyStageTransition<'a>, BodyStageTransitionError> {
        let transition =
            stage_body_stage_transition(coordinator, lease, candidate, parent_payload, edge)?;
        Ok(PreparedBodyStageTransition {
            _coordinator: coordinator,
            staged: transition.staged,
            edge,
            parent_ordinal: transition.parent_ordinal,
            child_ordinal: transition.child_ordinal,
            owner: transition.owner,
            child_slot: transition.child_slot,
            child_digest: transition.child_digest,
        })
    }

    fn prepare_authorized_validate_apply_transition<'a>(
        coordinator: &'a mut LifecycleCoordinator,
        lease: &TurnLease,
        validated_receipt: &ValidatedBodyReceipt,
        apply_effect: &AdapterEffect,
        apply_candidate: CandidateAdmission,
    ) -> Result<PreparedBodyStageTransition<'a>, BodyStageTransitionError> {
        let AdapterEffect::Apply {
            subject,
            certificate,
            ..
        } = apply_effect
        else {
            return Err(BodyStageTransitionError::InvalidValidationReceipt);
        };
        let durable = validated_receipt.durable();
        if durable.context_id() != certificate.round.context_id
            || durable.round() != certificate.proposal_round
            || durable.subject() != *subject
            || validated_receipt.execution_commitment() != certificate.execution_commitment
        {
            return Err(BodyStageTransitionError::InvalidValidationReceipt);
        }
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(coordinator.active_context, durable)
                .ok_or(BodyStageTransitionError::InvalidBodyFrameReference)?,
        );
        prepare_authorized_body_transition(
            coordinator,
            lease,
            apply_candidate,
            parent_payload,
            DurableContinuationEdge::ValidateToApply,
        )
    }

    pub(super) fn fetch_store_fixture(effect_limit: usize) -> FetchStoreFixture {
        fetch_store_fixture_with_authority(effect_limit, wire::GlobalPhase::Prepare)
    }

    #[allow(clippy::too_many_lines)]
    fn fetch_store_fixture_with_authority(
        effect_limit: usize,
        authority_phase: wire::GlobalPhase,
    ) -> FetchStoreFixture {
        let (verified, context) = verified_context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let tag = EventTag::new(context.height, round.view, Generation::new(1));
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"fetch-store block")),
            payload_hash: Hash::new(b"fetch-store payload"),
        };
        let execution_commitment = fixture_execution_commitment();
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: authority_phase,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x51],
        };
        let manifest = body_manifest(&verified, round, subject);
        let certified_sources = context
            .roster
            .iter()
            .map(|validator| validator.validator.clone())
            .collect::<Vec<_>>();
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(manifest.clone()),
            certified_sources: certified_sources.clone(),
            certificate: Some(certificate.clone()),
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let durable_receipt = durable_body_receipt(&verified, &manifest);
        let response = wire::CertifiedBodyResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"body-transition certified request",
            )),
            manifest: manifest.clone(),
            body: vec![0x51],
            responder: 0,
            signature: vec![0x52],
        };
        let fetch_owner = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 1)],
        )
        .expect("bind certified Fetch fixture")
        .pop()
        .expect("one certified Fetch owner");
        let fetch_pending = fetch_owner
            .pending_adapter_effect_binding(&fetch_effect)
            .expect("mint sealed certified Fetch binding");
        let fetch_digest = digest_from_hash(fetch_pending.exact_effect_identity());
        let store_pending = fetch_pending
            .project_certified_fetch_store_successor(&fetch_effect, &store_effect)
            .expect("project exact ordinal-free Store successor");
        let fetch_replay = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
            &fetch_effect,
            &response,
            &durable_receipt,
        )
        .expect("certified response projects exact Fetch replay evidence");
        let store_replay = fetch_replay
            .project_store_for_test(&store_effect, &durable_receipt)
            .expect("certified Fetch evidence projects exact Store replay evidence");
        let store_candidate = store_replay
            .project_candidate_for_test(&verified, &store_effect, &durable_receipt, &store_pending)
            .expect("canonical V1 evidence projects the Store candidate fixture");
        let replay = super::super::replay_authority::exact_durable_certified_fetch_record_fixture(
            lifecycle_context(&context),
            tag,
            certificate,
            manifest,
            certified_sources,
            &durable_receipt,
        );
        let fetch_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let parent = super::super::CandidateAdmission::new(
            replay.key,
            store_candidate.causal_root,
            replay.work_class,
            replay.stage,
            InitialLifecycleState::Ready,
            store_candidate.reconstruction_source,
            replay.payload,
            replay.authority,
            super::super::PhysicalGeometry::new(
                [super::super::PhysicalSlot::new(fetch_slot, fetch_digest)],
                [fetch_slot],
            ),
            None,
        );
        let mut coordinator = LifecycleCoordinator::new(
            lifecycle_context(&context),
            0,
            capacity_geometry(effect_limit),
        );
        let AdmissionDecision::Admitted {
            ordinal,
            producer_turn_ordinal: None,
            ..
        } = coordinator.admit(AdmissionRequest::Candidate(parent))
        else {
            panic!("admit Fetch parent fixture")
        };
        let record = &coordinator.records[&ordinal];
        let ready = super::super::SchedulerReadyInputs::new(record, None, [0; 6]);
        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("unique Fetch scheduler census");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("claim Fetch fixture")
        };
        FetchStoreFixture {
            coordinator,
            lease,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            store_candidate,
            store_replay,
        }
    }

    fn store_validate_fixture(
        effect_limit: usize,
        inherited_commitment: bool,
    ) -> StoreValidateFixture {
        store_validate_fixture_with_authority(
            effect_limit,
            inherited_commitment.then_some(wire::GlobalPhase::Prepare),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn store_validate_fixture_with_authority(
        effect_limit: usize,
        inherited_authority: Option<wire::GlobalPhase>,
    ) -> StoreValidateFixture {
        let FetchStoreFixture {
            verified,
            durable_receipt,
            store_effect,
            store_pending: certified_store_pending,
            store_candidate: certified_store_candidate,
            store_replay: certified_store_replay,
            ..
        } = fetch_store_fixture_with_authority(
            effect_limit,
            inherited_authority.unwrap_or(wire::GlobalPhase::Prepare),
        );
        let AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } = store_effect
        else {
            unreachable!("Fetch successor fixture is one Store effect")
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let (store_pending, store_candidate, validate_pending, validate_replay) =
            if inherited_authority.is_some() {
                let validate_pending = certified_store_pending
                    .project_store_validate_successor(&store_effect, &validate_effect)
                    .expect("project exact certified Validate successor");
                let validate_replay = DurableValidateReplayEvidenceV1::certified(
                    certified_store_replay
                        .project_validate(
                            &store_effect,
                            &durable_receipt,
                            &validate_effect,
                            &validate_pending,
                        )
                        .expect("certified Store evidence projects exact Validate evidence"),
                );
                (
                    certified_store_pending,
                    certified_store_candidate,
                    validate_pending,
                    validate_replay,
                )
            } else {
                let manifest = body_manifest(&verified, round, subject);
                let proposal = wire::Proposal {
                    round,
                    proposer: 0,
                    subject,
                    manifest: manifest.clone(),
                    justification: wire::ProposalJustification::ParentCommit(
                        wire::ParentCommitJustification { certificate: None },
                    ),
                    signature: vec![0x61],
                };
                let fetch_effect = AdapterEffect::FetchBody {
                    tag,
                    round,
                    subject,
                    manifest: Some(manifest),
                    certified_sources: Vec::new(),
                    certificate: None,
                };
                let mut fetch_owner = bind_adapter_effect_batch_ownership(
                    core::slice::from_ref(&fetch_effect),
                    vec![RuntimeEffectOwnership::fresh_for_test(tag, 2)],
                )
                .expect("bind remote-Proposal Fetch fixture")
                .pop()
                .expect("one remote-Proposal Fetch owner");
                assert!(
                    fetch_owner.bind_authenticated_remote_proposal_replay_for_test(
                        proposal,
                        &fetch_effect
                    )
                );
                let fetch_pending = fetch_owner
                    .pending_adapter_effect_binding(&fetch_effect)
                    .expect("mint sealed remote-Proposal Fetch binding");
                let fetch_replay = fetch_owner
                    .exact_remote_proposal_fetch_replay(&fetch_effect)
                    .expect("retain authenticated remote-Proposal replay evidence");
                let store_pending = fetch_pending
                    .project_proposal_fetch_store_successor(&fetch_effect, &store_effect)
                    .expect("project exact remote-Proposal Store successor");
                let stored_replay = fetch_replay
                    .project_exact_store(&store_effect, &store_pending)
                    .expect("project remote-Proposal Store replay evidence")
                    .bind_durable_body(&store_effect, &durable_receipt)
                    .expect("bind remote-Proposal Store to its body frame");
                let store_candidate = stored_replay
                    .project_candidate_for_test(
                        &verified,
                        &store_effect,
                        &durable_receipt,
                        &store_pending,
                    )
                    .expect("canonical Proposal evidence projects Store candidate");
                let validate_pending = store_pending
                    .project_store_validate_successor(&store_effect, &validate_effect)
                    .expect("project exact remote-Proposal Validate successor");
                let validate_replay = DurableValidateReplayEvidenceV1::remote_proposal(
                    stored_replay
                        .project_exact_validate(
                            &store_effect,
                            &durable_receipt,
                            &validate_effect,
                            &validate_pending,
                        )
                        .expect("project remote-Proposal Validate replay evidence"),
                );
                (
                    store_pending,
                    store_candidate,
                    validate_pending,
                    validate_replay,
                )
            };
        let validate_candidate = validate_replay
            .project_candidate_for_test(
                &verified,
                &validate_effect,
                &durable_receipt,
                &validate_pending,
            )
            .expect("canonical V1 evidence projects the Validate candidate fixture");
        let mut coordinator = LifecycleCoordinator::new(
            lifecycle_context(verified.context()),
            0,
            capacity_geometry(effect_limit),
        );
        let AdmissionDecision::Admitted {
            ordinal,
            producer_turn_ordinal: None,
            ..
        } = coordinator.admit(AdmissionRequest::Candidate(store_candidate))
        else {
            panic!("admit Store parent fixture")
        };
        let record = &coordinator.records[&ordinal];
        let ready = super::super::SchedulerReadyInputs::new(record, None, [0; 6]);
        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("unique Store scheduler census");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("claim Store fixture")
        };
        StoreValidateFixture {
            coordinator,
            lease,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            validate_effect,
            validate_pending,
            validate_candidate,
            validate_replay,
        }
    }

    fn validate_apply_fixture(
        effect_limit: usize,
        inherited_commitment: bool,
    ) -> ValidateApplyFixture {
        validate_apply_fixture_with_authority(
            effect_limit,
            inherited_commitment.then_some(wire::GlobalPhase::Prepare),
        )
    }

    fn validate_apply_fixture_with_authority(
        effect_limit: usize,
        inherited_authority: Option<wire::GlobalPhase>,
    ) -> ValidateApplyFixture {
        let StoreValidateFixture {
            verified,
            durable_receipt,
            validate_effect,
            validate_pending,
            validate_candidate,
            validate_replay,
            ..
        } = store_validate_fixture_with_authority(effect_limit, inherited_authority);
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = validate_effect
        else {
            unreachable!("Store successor fixture is one Validate effect")
        };
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let apply_effect = AdapterEffect::Apply {
            tag,
            subject,
            certificate: wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: fixture_execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5],
            },
        };
        let apply_pending = validate_pending
            .project_validate_apply_successor(&validate_effect, &apply_effect)
            .expect("project exact ordinal-free Apply successor");
        let validated_receipt = ValidatedBodyReceipt::for_test_with_commitment(
            durable_receipt.clone(),
            fixture_execution_commitment(),
        );
        let apply_candidate =
            super::super::replay_authority::exact_live_wal_body_successor_candidate_for_test(
                &verified,
                &validate_effect,
                &validate_pending,
                &apply_effect,
                &apply_pending,
                Some(&durable_receipt),
            )
            .expect("canonical live-WAL evidence projects the Apply candidate fixture");
        let mut coordinator = LifecycleCoordinator::new(
            lifecycle_context(verified.context()),
            0,
            capacity_geometry(effect_limit),
        );
        let AdmissionDecision::Admitted {
            ordinal,
            producer_turn_ordinal: None,
            ..
        } = coordinator.admit(AdmissionRequest::Candidate(validate_candidate.clone()))
        else {
            panic!("admit Validate parent fixture")
        };
        let record = &coordinator.records[&ordinal];
        let ready = super::super::SchedulerReadyInputs::new(record, Some(false), [0; 6]);
        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("unique Validate scheduler census");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("claim Validate fixture")
        };
        ValidateApplyFixture {
            coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validate_candidate,
            validate_replay,
            validated_receipt,
            apply_effect,
            apply_candidate,
        }
    }

    #[test]
    fn full_effect_capacity_stages_net_zero_success_and_drop_is_inert() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            verified,
            durable_receipt,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        assert_eq!(coordinator.capacity_used[&CapacityClass::Effect], 1);
        let before = format!("{coordinator:#?}");
        let expected_frame = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                lifecycle_context(verified.context()),
                &durable_receipt,
            )
            .expect("durable Fetch completion projects one body frame"),
        );
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            store_candidate,
            expected_frame,
            DurableContinuationEdge::FetchToStore,
        )
        .expect("Fetch release makes room for exact Store at full capacity");
        assert!(matches!(
            prepared.edge,
            DurableContinuationEdge::FetchToStore
        ));
        assert_eq!(prepared.parent_ordinal, lease.ordinal());
        assert_eq!(prepared.child_ordinal, lease.ordinal() + 1);
        assert_eq!(prepared.owner, lease.owner());
        assert_eq!(
            prepared.child_slot.capacity_class(),
            Some(CapacityClass::Effect)
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            1,
            "Fetch retirement and Store admission are net-zero"
        );
        assert_eq!(
            prepared.staged.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal].state,
            LifecycleState::Ready
        );
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .physical_slots
                .get(&prepared.child_slot),
            Some(&prepared.child_digest)
        );
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .episode
                .slot_universe,
            std::collections::BTreeSet::from([prepared.child_slot])
        );
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .episode
                .consumed_slots,
            std::collections::BTreeSet::from([prepared.child_slot])
        );
        assert_eq!(
            prepared.staged.durable_records[&prepared.parent_ordinal].payload,
            expected_frame
        );
        assert_eq!(
            prepared.staged.durable_records[&prepared.child_ordinal].payload,
            expected_frame
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn wrong_and_stale_fetch_leases_reject_without_coordinator_mutation() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        let before = format!("{coordinator:#?}");
        let mut wrong = lease.clone();
        wrong.work_class = LifecycleWorkClass::Store;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &wrong,
                store_candidate.clone(),
                store_candidate.payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::WrongParentShape)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);

        let mut stale = lease.clone();
        stale.id = super::super::LeaseId(lease.id().0 + 1);
        let parent_payload = store_candidate.payload;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &stale,
                store_candidate,
                parent_payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::StaleLease)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn foreign_store_projection_rejects_without_coordinator_mutation() {
        let FetchStoreFixture {
            coordinator,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            store_replay,
            ..
        } = fetch_store_fixture(1);
        let before = format!("{coordinator:#?}");
        let AdapterEffect::StoreBody {
            tag,
            round,
            mut subject,
        } = store_effect
        else {
            unreachable!("fixture Store effect")
        };
        subject.payload_hash = Hash::new(b"foreign Store body");
        let foreign = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        assert!(matches!(
            store_replay.project_candidate_for_test(
                &verified,
                &foreign,
                &durable_receipt,
                &store_pending,
            ),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn fetch_store_rejects_a_foreign_body_receipt_without_mutation() {
        let FetchStoreFixture {
            coordinator,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            store_replay,
            ..
        } = fetch_store_fixture(1);
        let AdapterEffect::StoreBody { round, subject, .. } = &store_effect else {
            unreachable!("fixture retains one Store effect")
        };
        let foreign_round = wire::ConsensusRound {
            view: round.view + 1,
            ..*round
        };
        let foreign_receipt = DurableBodyReceipt::for_test(
            verified.context().id(),
            foreign_round,
            *subject,
            durable_receipt.manifest_hash(),
        );
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            store_replay.project_candidate_for_test(
                &verified,
                &store_effect,
                &foreign_receipt,
                &store_pending,
            ),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn fetch_store_rejects_a_payload_free_parent_after_body_completion() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        coordinator
            .durable_records
            .get_mut(&lease.ordinal())
            .expect("claimed Fetch retains durable metadata")
            .payload = DurablePayloadReference::None;
        let before = format!("{coordinator:#?}");
        let parent_payload = store_candidate.payload;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                store_candidate,
                parent_payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::InvalidBodyFrameReference)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn staged_capacity_wait_leaves_fetch_parent_claimed() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        coordinator
            .capacity_geometry
            .limits
            .insert(CapacityClass::Effect, 0);
        let before_capacity = format!("{coordinator:#?}");
        let parent_payload = store_candidate.payload;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                store_candidate,
                parent_payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::ChildAdmission(decision))
                if matches!(*decision, AdmissionDecision::WaitForCapacity(_))
        ));
        assert_eq!(format!("{coordinator:#?}"), before_capacity);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    fn fetch_store_rejects_max_high_water_without_mutation() {
        let FetchStoreFixture {
            mut coordinator,
            lease,
            store_candidate,
            ..
        } = fetch_store_fixture(1);
        coordinator.high_water = u128::MAX;
        let before_ordinal = format!("{coordinator:#?}");
        let parent_payload = store_candidate.payload;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                store_candidate,
                parent_payload,
                DurableContinuationEdge::FetchToStore,
            ),
            Err(BodyStageTransitionError::OrdinalExhausted)
        ));
        assert_eq!(format!("{coordinator:#?}"), before_ordinal);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn full_effect_capacity_stages_exact_store_validate_cut_and_drop_is_inert() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_pending,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let expected_frame = validate_candidate.payload;
        let capacity_used_before = coordinator.capacity_used.clone();
        let capacity_generation_before = coordinator.capacity_generation.clone();
        let before = format!("{coordinator:#?}");
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            validate_candidate.clone(),
            expected_frame,
            DurableContinuationEdge::StoreToValidate,
        )
        .expect("Store release makes room for exact Validate at full capacity");

        assert!(matches!(
            prepared.edge,
            DurableContinuationEdge::StoreToValidate
        ));
        assert_eq!(prepared.parent_ordinal, lease.ordinal());
        assert_eq!(prepared.child_ordinal, lease.ordinal() + 1);
        assert_eq!(prepared.owner, lease.owner());
        assert_eq!(prepared.staged.high_water, prepared.child_ordinal);
        assert!(prepared.staged.active_lease.is_none());
        assert_eq!(
            prepared.child_slot,
            PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
        );
        assert_eq!(
            prepared.child_digest,
            digest_from_hash(validate_pending.exact_effect_identity())
        );

        let parent = &prepared.staged.records[&prepared.parent_ordinal];
        assert_eq!(parent.ordinal, prepared.parent_ordinal);
        assert_eq!(parent.owner, lease.owner());
        assert_eq!(parent.key, lease.key());
        assert_eq!(parent.work_class, LifecycleWorkClass::Store);
        assert_eq!(parent.stage.kind(), LifecycleStageKind::StoreBody);
        assert_eq!(
            parent.state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(parent.physical_slots, *lease.physical_slots());
        let parent_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(parent.episode.slot_universe, parent_slots);
        assert_eq!(parent.episode.consumed_slots, parent_slots);
        assert!(!prepared.staged.ready_index.contains(&parent.ordinal));
        assert_eq!(
            prepared.staged.key_index.get(&parent.key),
            Some(&parent.ordinal)
        );
        let parent_metadata = &prepared.staged.durable_records[&parent.ordinal];
        assert_eq!(
            parent_metadata.reconstruction_source,
            parent.owner.causal_root().digest()
        );
        assert_eq!(parent_metadata.payload, expected_frame);

        let child = &prepared.staged.records[&prepared.child_ordinal];
        assert_eq!(child.ordinal, prepared.child_ordinal);
        assert_eq!(child.owner, lease.owner());
        assert_eq!(child.key, validate_candidate.key);
        assert_eq!(child.work_class, LifecycleWorkClass::Validate);
        assert_eq!(child.stage.kind(), LifecycleStageKind::ValidateBody);
        assert_eq!(child.state, LifecycleState::Ready);
        assert_eq!(
            child.physical_slots,
            std::collections::BTreeMap::from([(prepared.child_slot, prepared.child_digest)])
        );
        assert_eq!(
            child.episode.slot_universe,
            std::collections::BTreeSet::from([prepared.child_slot])
        );
        assert_eq!(
            child.episode.consumed_slots,
            std::collections::BTreeSet::from([prepared.child_slot])
        );
        assert!(prepared.staged.ready_index.contains(&child.ordinal));
        assert_eq!(
            prepared.staged.key_index.get(&child.key),
            Some(&child.ordinal)
        );
        assert_eq!(
            prepared.staged.owner_index.get(&child.owner.causal_root()),
            Some(&child.owner)
        );
        assert!(
            prepared.staged.durable_records[&child.ordinal].matches_admission(&validate_candidate)
        );
        assert_eq!(
            prepared.staged.durable_records[&child.ordinal].payload, parent_metadata.payload,
            "Store and Validate retain one byte-identical body frame"
        );
        assert_eq!(child.key.context(), parent.key.context());
        assert_eq!(child.key.round(), parent.key.round());
        assert_eq!(child.key.proposal_round(), parent.key.proposal_round());
        assert_eq!(child.key.subject(), parent.key.subject());
        assert_eq!(
            child.key.execution_commitment(),
            parent.key.execution_commitment()
        );
        assert_eq!(parent.key.phase(), LifecyclePhase::Store);
        assert_eq!(child.key.phase(), LifecyclePhase::Validate);

        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            capacity_used_before[&CapacityClass::Effect]
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Effect],
            capacity_generation_before[&CapacityClass::Effect] + 1
        );
        for class in CapacityClass::ALL
            .into_iter()
            .filter(|class| *class != CapacityClass::Effect)
        {
            assert_eq!(
                prepared.staged.capacity_used[&class],
                capacity_used_before[&class]
            );
            assert_eq!(
                prepared.staged.capacity_generation[&class],
                capacity_generation_before[&class]
            );
        }

        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn store_validate_accepts_exact_no_commitment_lineage() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, false);
        assert_eq!(lease.key().execution_commitment(), None);
        let before = format!("{coordinator:#?}");
        let parent_payload = validate_candidate.payload;
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            validate_candidate,
            parent_payload,
            DurableContinuationEdge::StoreToValidate,
        )
        .expect("ordinary body statement retains its exact absent commitment");
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .key
                .execution_commitment(),
            None
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn store_validate_rejects_a_substituted_frame_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            durable_receipt,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let substituted = DurableBodyReceipt::for_test(
            durable_receipt.context_id(),
            durable_receipt.round(),
            durable_receipt.subject(),
            HashOf::from_untyped_unchecked(Hash::new(b"substituted body manifest")),
        );
        let substituted_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(coordinator.active_context, &substituted)
                .expect("substituted receipt still projects a structurally valid frame"),
        );
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate,
                substituted_payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::InvalidBodyFrameReference)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn wrong_and_stale_store_leases_reject_without_coordinator_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let before = format!("{coordinator:#?}");
        let mut wrong = lease.clone();
        wrong.work_class = LifecycleWorkClass::Validate;
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &wrong,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::WrongParentShape)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);

        let mut stale = lease.clone();
        stale.id = super::super::LeaseId(lease.id().0 + 1);
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &stale,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::StaleLease)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn wrong_validate_effect_binding_and_owner_reject_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            verified,
            durable_receipt,
            store_effect,
            store_pending,
            validate_effect,
            validate_pending,
            validate_candidate,
            validate_replay,
        } = store_validate_fixture(1, true);
        let before = format!("{coordinator:#?}");
        let (tag, round, mut wrong_subject) = match &validate_effect {
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } => (*tag, *round, *subject),
            _ => unreachable!("fixture Validate effect"),
        };
        wrong_subject.payload_hash = Hash::new(b"foreign Validate body");
        let wrong_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject: wrong_subject,
        };
        assert!(matches!(
            validate_replay.project_candidate_for_test(
                &verified,
                &wrong_effect,
                &durable_receipt,
                &validate_pending,
            ),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert!(matches!(
            validate_replay.project_candidate_for_test(
                &verified,
                &validate_effect,
                &durable_receipt,
                &store_pending,
            ),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);

        let foreign_owner_tag = EventTag::new(
            tag.height(),
            tag.view(),
            Generation::new(
                tag.generation()
                    .get()
                    .checked_add(1)
                    .expect("fixture generation remains bounded"),
            ),
        );
        let foreign_store_owner = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&store_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(
                foreign_owner_tag,
                99,
            )],
        )
        .expect("bind foreign Store owner")
        .pop()
        .expect("one foreign Store owner");
        let foreign_store_pending = foreign_store_owner
            .pending_adapter_effect_binding(&store_effect)
            .expect("mint foreign Store pending binding");
        let foreign_validate_pending = foreign_store_pending
            .project_store_validate_successor(&store_effect, &validate_effect)
            .expect("project foreign Validate pending binding");
        assert_ne!(
            foreign_validate_pending.causal_lifecycle_key(),
            validate_pending.causal_lifecycle_key()
        );
        let parent_payload = validate_candidate.payload;
        let mut foreign_candidate = validate_candidate;
        foreign_candidate.causal_root = super::super::CausalRoot::new(digest_from_hash(
            foreign_validate_pending.causal_lifecycle_key(),
        ));
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                foreign_candidate,
                parent_payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::ForeignSuccessorOwner)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn foreign_store_lineage_rejects_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            mut lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let incumbent_key = lease.key();
        let foreign_key = super::super::LifecycleKey::new(
            incumbent_key.context(),
            incumbent_key.round(),
            incumbent_key.proposal_round(),
            Some(super::super::LifecycleDigest::new([0xF1; 32])),
            LifecyclePhase::Store,
            incumbent_key.execution_commitment(),
        );
        lease.key = foreign_key;
        coordinator.active_lease = Some(lease.clone());
        assert_eq!(
            coordinator.key_index.remove(&incumbent_key),
            Some(lease.ordinal())
        );
        assert_eq!(
            coordinator.key_index.insert(foreign_key, lease.ordinal()),
            None
        );
        coordinator
            .records
            .get_mut(&lease.ordinal())
            .expect("claimed Store record")
            .key = foreign_key;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::ForeignSuccessorLineage)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn store_validate_rejects_max_high_water_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        coordinator.high_water = u128::MAX;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::OrdinalExhausted)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    fn store_validate_rejects_capacity_generation_overflow_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        coordinator
            .capacity_generation
            .insert(CapacityClass::Effect, u64::MAX);
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::InvalidCapacityTransition)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    fn corrupt_store_reconstruction_source_rejects_without_mutation() {
        let StoreValidateFixture {
            mut coordinator,
            lease,
            validate_candidate,
            ..
        } = store_validate_fixture(1, true);
        let corrupt = super::super::LifecycleDigest::new([0xD3; 32]);
        assert_ne!(corrupt, lease.owner().causal_root().digest());
        coordinator
            .durable_records
            .get_mut(&lease.ordinal())
            .expect("Store durable metadata")
            .reconstruction_source = corrupt;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_body_transition(
                &mut coordinator,
                &lease,
                validate_candidate.clone(),
                validate_candidate.payload,
                DurableContinuationEdge::StoreToValidate,
            ),
            Err(BodyStageTransitionError::StaleLease)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
        assert_eq!(coordinator.active_lease, Some(lease));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn full_effect_capacity_stages_exact_validate_apply_cut_and_drop_is_inert() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let expected_frame = apply_candidate.payload;
        let capacity_used_before = coordinator.capacity_used.clone();
        let capacity_generation_before = coordinator.capacity_generation.clone();
        let before = format!("{coordinator:#?}");
        let prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate.clone(),
        )
        .expect("Validate release makes room for exact Apply at full capacity");

        assert!(matches!(
            prepared.edge,
            DurableContinuationEdge::ValidateToApply
        ));
        assert_eq!(prepared.parent_ordinal, lease.ordinal());
        assert_eq!(prepared.child_ordinal, lease.ordinal() + 1);
        assert_eq!(prepared.owner, lease.owner());
        let parent = &prepared.staged.records[&prepared.parent_ordinal];
        let child = &prepared.staged.records[&prepared.child_ordinal];
        assert_eq!(parent.work_class, LifecycleWorkClass::Validate);
        assert_eq!(parent.stage.kind(), LifecycleStageKind::ValidateBody);
        assert_eq!(
            parent.state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(child.owner, lease.owner());
        assert_eq!(child.key, apply_candidate.key);
        assert_eq!(child.work_class, LifecycleWorkClass::Apply);
        assert_eq!(child.stage.kind(), LifecycleStageKind::ApplyDecision);
        assert_eq!(child.state, LifecycleState::Ready);
        assert_eq!(child.key.context(), parent.key.context());
        assert_eq!(child.key.round(), parent.key.round());
        assert_eq!(child.key.proposal_round(), parent.key.proposal_round());
        assert_eq!(child.key.subject(), parent.key.subject());
        assert_eq!(
            child.key.execution_commitment(),
            parent.key.execution_commitment()
        );
        assert!(child.key.execution_commitment().is_some());
        assert_eq!(
            child.physical_slots,
            std::collections::BTreeMap::from([(prepared.child_slot, prepared.child_digest)])
        );
        assert!(prepared.staged.ready_index.contains(&child.ordinal));
        assert!(
            prepared.staged.durable_records[&child.ordinal].matches_admission(&apply_candidate)
        );
        assert_eq!(
            prepared.staged.durable_records[&parent.ordinal].payload,
            expected_frame
        );
        assert_eq!(
            prepared.staged.durable_records[&child.ordinal].payload, expected_frame,
            "Validate and Apply retain one byte-identical body frame"
        );
        assert_eq!(
            prepared.staged.durable_records[&parent.ordinal].continuation,
            DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, child.ordinal,)
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            capacity_used_before[&CapacityClass::Effect]
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Effect],
            capacity_generation_before[&CapacityClass::Effect] + 1
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[allow(clippy::too_many_lines)]
    fn assert_validate_sign_transition_is_exact(phase: wire::GlobalPhase) {
        let inherited = match phase {
            wire::GlobalPhase::Prepare => None,
            wire::GlobalPhase::Commit => Some(wire::GlobalPhase::Prepare),
        };
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validated_receipt,
            ..
        } = validate_apply_fixture_with_authority(1, inherited);
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &validate_effect
        else {
            unreachable!("fixture retains one Validate effect")
        };
        let (tag, round, subject) = (*tag, *round, *subject);
        let sign_effect = AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(wire::Vote {
                round,
                proposal_round: round,
                phase,
                subject,
                execution_commitment: validated_receipt.execution_commitment(),
                signer: 0,
                signature: Vec::new(),
            }),
        };
        let sign_pending = match phase {
            wire::GlobalPhase::Prepare => validate_pending
                .project_validate_sign_prepare_successor(&validate_effect, &sign_effect),
            wire::GlobalPhase::Commit => validate_pending
                .project_validate_sign_commit_successor(&validate_effect, &sign_effect),
        }
        .expect("sealed Validate authority projects its exact Sign successor");
        let expected_edge = match phase {
            wire::GlobalPhase::Prepare => DurableContinuationEdge::ValidateToSignPrepare,
            wire::GlobalPhase::Commit => DurableContinuationEdge::ValidateToSignCommit,
        };
        let expected_stage = match phase {
            wire::GlobalPhase::Prepare => LifecycleStageKind::SignPrepareVote,
            wire::GlobalPhase::Commit => LifecycleStageKind::SignCommitVote,
        };
        let before = format!("{coordinator:#?}");
        let effect_used_before = coordinator.capacity_used[&CapacityClass::Effect];
        let effect_generation_before = coordinator.capacity_generation[&CapacityClass::Effect];
        let sign_candidate =
            super::super::replay_authority::exact_live_wal_body_successor_candidate_for_test(
                &verified,
                &validate_effect,
                &validate_pending,
                &sign_effect,
                &sign_pending,
                None,
            )
            .expect("canonical live-WAL evidence projects the Sign candidate");
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                coordinator.active_context,
                validated_receipt.durable(),
            )
            .expect("Validate fixture retains one exact body frame"),
        );
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            sign_candidate,
            parent_payload,
            expected_edge,
        )
        .expect("stage exact Validate-to-Sign durable cut");
        assert_eq!(prepared.edge, expected_edge);
        assert_eq!(prepared.parent_ordinal, lease.ordinal());
        assert_eq!(prepared.child_ordinal, lease.ordinal() + 1);
        assert_eq!(prepared.owner, lease.owner());
        assert_eq!(
            prepared.staged.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        let child = &prepared.staged.records[&prepared.child_ordinal];
        assert_eq!(child.owner, lease.owner());
        assert_eq!(child.work_class, LifecycleWorkClass::SignVote);
        assert_eq!(child.stage.kind(), expected_stage);
        assert_eq!(child.state, LifecycleState::Ready);
        assert_eq!(
            prepared.staged.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::successor(expected_edge, child.ordinal)
        );
        assert!(matches!(
            prepared.staged.durable_records[&lease.ordinal()].payload,
            DurablePayloadReference::BodyFrame(_)
        ));
        assert_eq!(
            prepared.staged.durable_records[&child.ordinal].payload,
            DurablePayloadReference::None
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            effect_used_before
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Effect],
            effect_generation_before + 1
        );
        super::super::ledger::LifecycleLedgerV1::from_coordinator(&prepared.staged)
            .expect("typed Validate-to-Sign edge projects into LedgerV1");
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn validate_sign_prepare_and_commit_stage_exact_net_zero_cuts() {
        assert_validate_sign_transition_is_exact(wire::GlobalPhase::Prepare);
        assert_validate_sign_transition_is_exact(wire::GlobalPhase::Commit);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn rejected_validate_reservation_converts_into_exact_report_capacity() {
        // The complete sealed report wrapper spans private adapter and registry
        // fixture state owned by their respective modules. Adding a test-only
        // constructor here would reopen the boundary this tranche closes.
        // Adapter tests cover exact registered-Prepare report preview/drop,
        // registry tests cover exact Ready carrier and foreign-height rejection,
        // and this test joins canonical report evidence to the shared staging
        // core while proving raw report admission is rejected and drop is inert.
        let ValidateApplyFixture {
            mut coordinator,
            mut lease,
            verified,
            validate_effect,
            validate_pending,
            validate_replay,
            validated_receipt,
            ..
        } = validate_apply_fixture_with_authority(1, Some(wire::GlobalPhase::Prepare));
        let AdapterEffect::ValidateBody {
            tag: _,
            round,
            subject,
        } = &validate_effect
        else {
            unreachable!("fixture retains one Validate effect")
        };
        let report_effect = AdapterEffect::ReportInvalidCertifiedBody {
            subject: *subject,
            certificate: wire::QuorumCertificate {
                round: *round,
                proposal_round: *round,
                phase: wire::GlobalPhase::Prepare,
                subject: *subject,
                execution_commitment: validated_receipt.execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x51],
            },
        };
        let report_pending = validate_pending
            .project_validate_report_invalid_certified_body_successor(
                &validate_effect,
                &report_effect,
            )
            .expect("Prepare-authorized Validate projects its exact report");
        lease.output_reservation = Some(super::super::schema::LeaseCapacityReservation::new(
            CapacityClass::Consensus,
            coordinator.capacity_generation[&CapacityClass::Consensus],
        ));
        coordinator.active_lease = Some(lease.clone());
        let effect_used_before = coordinator.capacity_used[&CapacityClass::Effect];
        let effect_generation_before = coordinator.capacity_generation[&CapacityClass::Effect];
        let consensus_used_before = coordinator.capacity_used[&CapacityClass::Consensus];
        let consensus_generation_before =
            coordinator.capacity_generation[&CapacityClass::Consensus];
        let before = format!("{coordinator:#?}");
        let report_candidate =
            super::super::replay_authority::exact_invalid_body_report_candidate_for_test(
                &verified,
                &validate_replay,
                &validate_effect,
                &validate_pending,
                validated_receipt.durable(),
                &report_effect,
                &report_pending,
            )
            .expect("canonical rejection evidence projects the report candidate");
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                coordinator.active_context,
                validated_receipt.durable(),
            )
            .expect("rejected Validate fixture retains one exact body frame"),
        );
        let prepared = prepare_authorized_body_transition(
            &mut coordinator,
            &lease,
            report_candidate,
            parent_payload,
            DurableContinuationEdge::ValidateToInvalidBodyReport,
        )
        .expect("convert the reserved rejected Validate into one report child");
        let child = &prepared.staged.records[&prepared.child_ordinal];
        assert_eq!(child.work_class, LifecycleWorkClass::InvalidBodyReport);
        assert_eq!(child.stage.kind(), LifecycleStageKind::ReportInvalidBody);
        assert_eq!(child.state, LifecycleState::Ready);
        assert_eq!(child.owner, lease.owner());
        assert_eq!(
            prepared.child_slot.capacity_class(),
            Some(CapacityClass::Consensus)
        );
        assert_eq!(
            prepared.staged.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::successor(
                DurableContinuationEdge::ValidateToInvalidBodyReport,
                child.ordinal,
            )
        );
        assert!(matches!(
            prepared.staged.durable_records[&lease.ordinal()].payload,
            DurablePayloadReference::BodyFrame(_)
        ));
        assert_eq!(
            prepared.staged.durable_records[&child.ordinal].payload,
            DurablePayloadReference::None
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Effect],
            effect_used_before - 1
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Effect],
            effect_generation_before + 1
        );
        assert_eq!(
            prepared.staged.capacity_used[&CapacityClass::Consensus],
            consensus_used_before + 1
        );
        assert_eq!(
            prepared.staged.capacity_generation[&CapacityClass::Consensus],
            consensus_generation_before
        );
        super::super::ledger::LifecycleLedgerV1::from_coordinator(&prepared.staged)
            .expect("typed Validate-to-report edge projects into LedgerV1");
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    fn assert_validate_no_successor_cut_is_exact(rejected: bool) {
        let ValidateApplyFixture {
            mut coordinator,
            mut lease,
            validated_receipt,
            ..
        } = validate_apply_fixture(1, false);
        if rejected {
            lease.output_reservation = Some(super::super::schema::LeaseCapacityReservation::new(
                CapacityClass::Consensus,
                coordinator.capacity_generation[&CapacityClass::Consensus],
            ));
            coordinator.active_lease = Some(lease.clone());
        }
        let effect_used_before = coordinator.capacity_used[&CapacityClass::Effect];
        let effect_generation_before = coordinator.capacity_generation[&CapacityClass::Effect];
        let consensus_used_before = coordinator.capacity_used[&CapacityClass::Consensus];
        let consensus_generation_before =
            coordinator.capacity_generation[&CapacityClass::Consensus];
        let high_water_before = coordinator.high_water;
        let before = format!("{coordinator:#?}");
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                coordinator.active_context,
                validated_receipt.durable(),
            )
            .expect("terminal Validate fixture retains its exact body frame"),
        );
        let transition =
            stage_validate_no_successor_transition(&coordinator, &lease, parent_payload, rejected)
                .expect("stage exact terminal Validate with no successor");
        assert_eq!(transition.parent_ordinal, lease.ordinal());
        assert_eq!(transition.released_consensus_reservation, rejected);
        assert_eq!(transition.staged.high_water, high_water_before);
        assert_eq!(
            transition.staged.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(
            transition.staged.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::AdvancedNoSuccessor
        );
        assert_eq!(
            transition.staged.durable_records[&lease.ordinal()].payload,
            DurablePayloadReference::BodyFrame(
                projection::durable_body_frame_reference(
                    coordinator.active_context,
                    validated_receipt.durable(),
                )
                .expect("terminal Validate retains its exact body frame"),
            )
        );
        assert_eq!(
            transition.staged.capacity_used[&CapacityClass::Effect],
            effect_used_before - 1
        );
        assert_eq!(
            transition.staged.capacity_generation[&CapacityClass::Effect],
            effect_generation_before + 1
        );
        assert_eq!(
            transition.staged.capacity_used[&CapacityClass::Consensus],
            consensus_used_before
        );
        assert_eq!(
            transition.staged.capacity_generation[&CapacityClass::Consensus],
            consensus_generation_before + u64::from(rejected)
        );
        super::super::ledger::LifecycleLedgerV1::from_coordinator(&transition.staged)
            .expect("typed Validate no-successor tombstone projects into LedgerV1");
        drop(transition);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn validated_and_rejected_no_effect_cuts_release_exact_capacity() {
        // The registry test pins all four accepted preview discriminators and
        // rejects every Busy/Apply/Persist/Report branch. This lower-level cut
        // then proves the permit-bound projection's only two capacity outcomes
        // without adding a test constructor for the private dual-borrow preview.
        assert_validate_no_successor_cut_is_exact(false);
        assert_validate_no_successor_cut_is_exact(true);

        let ValidateApplyFixture {
            coordinator,
            lease,
            validated_receipt,
            ..
        } = validate_apply_fixture(1, false);
        let parent_payload = DurablePayloadReference::BodyFrame(
            projection::durable_body_frame_reference(
                coordinator.active_context,
                validated_receipt.durable(),
            )
            .expect("terminal Validate fixture retains its exact body frame"),
        );
        assert!(matches!(
            stage_validate_no_successor_transition(&coordinator, &lease, parent_payload, true,),
            Err(BodyStageTransitionError::InvalidOutputReservation)
        ));
    }

    #[test]
    fn validate_apply_acquires_commit_authority_for_ordinary_validation() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, false);
        assert_eq!(lease.key().execution_commitment(), None);
        let before = format!("{coordinator:#?}");
        let prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate,
        )
        .expect("ordinary Validate may acquire exact Commit authority");
        assert!(
            prepared.staged.records[&prepared.child_ordinal]
                .key
                .execution_commitment()
                .is_some()
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn advanced_validate_link_stutters_and_recovers_its_exact_apply() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            validate_candidate,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let retry = AdmissionRequest::Candidate(validate_candidate);
        let mut prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate,
        )
        .expect("stage exact durable Validate-to-Apply link");
        assert!(matches!(
            prepared.staged.admit(retry),
            AdmissionDecision::StutterTerminal { owner } if owner == lease.owner()
        ));

        let ledger = super::super::ledger::LifecycleLedgerV1::from_coordinator(&prepared.staged)
            .expect("project linked body rows into LedgerV1");
        let physical_universes = prepared
            .staged
            .records
            .iter()
            .map(|(ordinal, record)| (*ordinal, record.episode.slot_universe.clone()))
            .collect();
        let snapshot = ledger
            .recovery_snapshot(physical_universes)
            .expect("decode an authenticated linked recovery snapshot");
        let authority = prepared.staged.episode_authority.clone();
        let mut recovered =
            LifecycleCoordinator::new_with_authority(authority.clone(), snapshot.high_water);
        recovered.reconcile_restart(snapshot.clone());
        assert_eq!(recovered.fault(), None);
        assert_eq!(
            recovered.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::successor(
                DurableContinuationEdge::ValidateToApply,
                lease.ordinal() + 1,
            )
        );

        let mut missing_link = snapshot;
        missing_link
            .records
            .iter_mut()
            .find(|record| record.ordinal == lease.ordinal())
            .expect("recovery contains terminal Validate parent")
            .continuation = DurableContinuation::None;
        let mut rejected =
            LifecycleCoordinator::new_with_authority(authority, missing_link.high_water);
        rejected.reconcile_restart(missing_link);
        assert_eq!(rejected.fault(), Some(CoordinatorFault::RecoveryRejected));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn durable_open_joins_terminal_validate_to_authenticated_apply() {
        let temporary = tempfile::tempdir().expect("temporary lifecycle roots");
        let ledger_root = temporary.path().join("ledger");
        let body_root = temporary.path().join("bodies");
        let missing_payload_root = temporary.path().join("missing-payloads");
        let exact_payload_root = temporary.path().join("exact-payloads");
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate.clone(),
        )
        .expect("stage exact durable Validate-to-Apply link");
        let authority = prepared.staged.episode_authority.clone();
        let ledger = super::super::ledger::LifecycleLedgerV1::from_coordinator(&prepared.staged)
            .expect("project exact linked ledger");
        let (ledger_store, empty) = super::super::ledger::LifecycleLedgerStoreV1::open(
            &ledger_root,
            lifecycle_context(verified.context()),
        )
        .expect("open durable lifecycle ledger");
        assert!(empty.records().is_empty());
        ledger_store
            .persist(&ledger)
            .expect("persist linked Validate-to-Apply ledger");
        drop(ledger_store);

        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            &body_root,
            verified.context().clone(),
        )
        .expect("open exact-context body store");
        let signer = KeyPair::try_from_seed(vec![250; 32], Algorithm::BlsNormal)
            .expect("deterministic empty-cut signer");
        let (mut missing_payload_store, missing_payloads) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &missing_payload_root,
                verified.context(),
            )
            .expect("open empty missing-candidate payload store");
        let missing_payloads = missing_payloads
            .authenticate(&verified, &signer, &body_store)
            .expect("authenticate empty Serve payload cut");
        let missing_cut =
            super::super::AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(
                ledger.clone(),
                [],
                [],
                missing_payloads,
            )
            .expect("assemble missing Apply recovery cut");
        assert!(
            LifecycleCoordinator::open_with_authority(
                authority.clone(),
                &ledger_root,
                &mut missing_payload_store,
                missing_cut,
            )
            .is_err(),
            "a live Apply successor requires exact authenticated recovery coverage"
        );

        let (mut exact_payload_store, exact_payloads) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &exact_payload_root,
                verified.context(),
            )
            .expect("open empty exact-candidate payload store");
        let exact_payloads = exact_payloads
            .authenticate(&verified, &signer, &body_store)
            .expect("authenticate second empty Serve payload cut");
        let exact_cut = super::super::AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(
            ledger,
            [apply_candidate],
            [],
            exact_payloads,
        )
        .expect("assemble exact Apply recovery cut");
        let restarted = LifecycleCoordinator::open_with_authority(
            authority,
            &ledger_root,
            &mut exact_payload_store,
            exact_cut,
        )
        .expect("linked terminal Validate and authenticated live Apply reopen exactly");
        assert_eq!(restarted.fault(), None);
        assert_eq!(
            restarted.records[&lease.ordinal()].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        );
        assert_eq!(
            restarted.durable_records[&lease.ordinal()].continuation,
            DurableContinuation::successor(
                DurableContinuationEdge::ValidateToApply,
                lease.ordinal() + 1,
            )
        );
        assert_eq!(
            restarted.records[&(lease.ordinal() + 1)].state,
            LifecycleState::Ready
        );
    }

    #[test]
    fn validate_apply_rejects_wrong_binding_and_foreign_commitment_without_mutation() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let before = format!("{coordinator:#?}");
        assert!(
            super::super::replay_authority::exact_live_wal_body_successor_candidate_for_test(
                &verified,
                &validate_effect,
                &validate_pending,
                &apply_effect,
                &validate_pending,
                Some(validated_receipt.durable()),
            )
            .is_none()
        );
        assert_eq!(format!("{coordinator:#?}"), before);

        let AdapterEffect::Apply {
            tag,
            subject,
            mut certificate,
        } = apply_effect
        else {
            unreachable!("fixture Apply effect")
        };
        certificate.execution_commitment =
            wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"foreign Validate-Apply state root"),
                Hash::new(b"foreign Validate-Apply events root"),
                Hash::new(b"foreign Validate-Apply trace root"),
                2,
                Hash::new(b"foreign Validate-Apply fee summary"),
            );
        let foreign_apply = AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        };
        assert!(
            validate_pending
                .project_validate_apply_successor(&validate_effect, &foreign_apply)
                .is_none(),
            "Prepare-authorized Validate must reject a different Commit result"
        );
        assert!(matches!(
            prepare_authorized_validate_apply_transition(
                &mut coordinator,
                &lease,
                &validated_receipt,
                &foreign_apply,
                apply_candidate,
            ),
            Err(BodyStageTransitionError::InvalidValidationReceipt)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn ordinary_validate_receipt_rejects_self_consistent_foreign_apply_binding() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            verified,
            validate_effect,
            validate_pending,
            validated_receipt,
            apply_effect,
            ..
        } = validate_apply_fixture(1, false);
        let AdapterEffect::Apply {
            tag,
            subject,
            mut certificate,
        } = apply_effect
        else {
            unreachable!("fixture Apply effect")
        };
        certificate.execution_commitment =
            wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"ordinary forged state root"),
                Hash::new(b"ordinary forged events root"),
                Hash::new(b"ordinary forged trace root"),
                3,
                Hash::new(b"ordinary forged fee summary"),
            );
        let foreign_apply = AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        };
        let foreign_pending = validate_pending
            .project_validate_apply_successor(&validate_effect, &foreign_apply)
            .expect("ordinary lineage alone permits one internally exact Commit binding");
        let foreign_candidate =
            super::super::replay_authority::exact_live_wal_body_successor_candidate_for_test(
                &verified,
                &validate_effect,
                &validate_pending,
                &foreign_apply,
                &foreign_pending,
                Some(validated_receipt.durable()),
            )
            .expect("self-consistent foreign Apply has exact test WAL evidence");
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_validate_apply_transition(
                &mut coordinator,
                &lease,
                &validated_receipt,
                &foreign_apply,
                foreign_candidate,
            ),
            Err(BodyStageTransitionError::InvalidValidationReceipt)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn commit_authorized_validate_retains_only_the_exact_commit_result() {
        let ValidateApplyFixture {
            mut coordinator,
            lease,
            validate_effect,
            validate_pending,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture_with_authority(1, Some(wire::GlobalPhase::Commit));
        assert!(lease.key().execution_commitment().is_some());
        let before = format!("{coordinator:#?}");
        let prepared = prepare_authorized_validate_apply_transition(
            &mut coordinator,
            &lease,
            &validated_receipt,
            &apply_effect,
            apply_candidate,
        )
        .expect("exact Commit-authorized Validate retains its Apply authority");
        assert_eq!(
            prepared.staged.records[&prepared.child_ordinal]
                .key
                .execution_commitment(),
            lease.key().execution_commitment()
        );
        drop(prepared);
        assert_eq!(format!("{coordinator:#?}"), before);

        let AdapterEffect::Apply {
            tag,
            subject,
            mut certificate,
        } = apply_effect
        else {
            unreachable!("fixture Apply effect")
        };
        certificate.execution_commitment =
            wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"changed retained Commit state root"),
                Hash::new(b"changed retained Commit events root"),
                Hash::new(b"changed retained Commit trace root"),
                4,
                Hash::new(b"changed retained Commit fee summary"),
            );
        let changed_apply = AdapterEffect::Apply {
            tag,
            subject,
            certificate,
        };
        assert!(
            validate_pending
                .project_validate_apply_successor(&validate_effect, &changed_apply)
                .is_none(),
            "Commit authority may retain only its exact statement"
        );
        assert_eq!(format!("{coordinator:#?}"), before);
    }

    #[test]
    fn validate_apply_rejects_corrupt_parent_commitment_lineage_without_mutation() {
        let ValidateApplyFixture {
            mut coordinator,
            mut lease,
            validated_receipt,
            apply_effect,
            apply_candidate,
            ..
        } = validate_apply_fixture(1, true);
        let incumbent_key = lease.key();
        let foreign_key = super::super::LifecycleKey::new(
            incumbent_key.context(),
            incumbent_key.round(),
            incumbent_key.proposal_round(),
            incumbent_key.subject(),
            LifecyclePhase::Validate,
            Some(super::super::LifecycleDigest::new([0xE1; 32])),
        );
        assert_ne!(
            foreign_key.execution_commitment(),
            incumbent_key.execution_commitment()
        );
        lease.key = foreign_key;
        coordinator.active_lease = Some(lease.clone());
        assert_eq!(
            coordinator.key_index.remove(&incumbent_key),
            Some(lease.ordinal())
        );
        assert_eq!(
            coordinator.key_index.insert(foreign_key, lease.ordinal()),
            None
        );
        coordinator
            .records
            .get_mut(&lease.ordinal())
            .expect("claimed Validate record")
            .key = foreign_key;
        let before = format!("{coordinator:#?}");
        assert!(matches!(
            prepare_authorized_validate_apply_transition(
                &mut coordinator,
                &lease,
                &validated_receipt,
                &apply_effect,
                apply_candidate,
            ),
            Err(BodyStageTransitionError::ForeignSuccessorLineage)
        ));
        assert_eq!(format!("{coordinator:#?}"), before);
    }
}
