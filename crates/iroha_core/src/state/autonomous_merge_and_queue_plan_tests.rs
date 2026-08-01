    #[expect(
        clippy::too_many_lines,
        reason = "the fixture assembles one complete availability-certified autonomous source"
    )]
    fn autonomous_merge_source_for_queue_plan_admission_test(
        state: &State,
        binding: &crate::torii_proxy::QueuePlanAdmissionBindingV2,
        entrypoint: TransactionEntrypoint,
        routing_plan: crate::queue::RoutingPlan,
        activation_validator_keypairs: &[KeyPair],
    ) -> MergeExecutionSource {
        let coordinator = binding
            .admission_context
            .route_incarnations
            .first()
            .expect("fixture binding has a coordinator");
        let proposal_height = binding.admission_context.proposal_height;
        // This fixture has one active execution lane, so production merge
        // validation binds autonomous certification to the global commit
        // topology rather than the QueuePlan admission committee.
        let mut validator_set = state.commit_topology_snapshot();
        validator_set.sort();
        validator_set.dedup();
        assert!(
            !validator_set.is_empty(),
            "fixture activation committee must not be empty"
        );
        let validator_count =
            u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
        let min_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
            validator_set.len(),
        ))
        .expect("fixture quorum fits u32");
        let lane_block_height = 1;
        let lane_block_view = 0;
        let entrypoint_hash = Hash::from(entrypoint.hash());
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: coordinator.leg.route.lane_id,
            dataspace_id: coordinator.leg.route.dataspace_id,
            lane_incarnation: coordinator.lane_incarnation,
            proposal_height,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height,
            lane_block_view,
            subject_hash: Hash::new(b"queue-plan-pre-carrier-autonomous-subject"),
            payload_ownership_hash: Hash::new(b"queue-plan-pre-carrier-autonomous-ownership"),
            rbc_instance_hash: Hash::new(b"queue-plan-pre-carrier-autonomous-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![entrypoint_hash],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count,
            min_quorum,
            qc_mode_tag: "permissioned:queue-plan-pre-carrier-autonomous".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();

        let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
            entrypoint.clone(),
        ));
        let reservation = crate::queue::LaneQueueReservationKeyV2 {
            version: crate::queue::LaneQueueReservationKeyV2::VERSION,
            signed_transaction_hash: accepted.hash(),
            entrypoint_hash: entrypoint.hash(),
            queue_plan_admission_binding_hash: binding.canonical_hash(),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            proposal_height,
            lane_block_height,
            lane_block_view,
            reservation_owner_hash: Hash::new(
                b"queue-plan-pre-carrier-autonomous-reservation-owner",
            ),
            proposal_identity_hash: proposal.proposal_hash,
        };
        let producer =
            crate::lane_consensus::deterministic_lane_author(&validator_set, lane_block_height)
                .cloned()
                .expect("fixture activation committee has a deterministic lane author");
        let producer_keypair = activation_validator_keypairs
            .iter()
            .find(|keypair| keypair.public_key() == producer.public_key())
            .expect("fixture retains the deterministic producer key");
        let chain_id_hash = Hash::new(state.chain_id.clone().into_inner().as_bytes());
        let epoch =
            crate::sumeragi::epoch_for_height_from_world(&state.world.view(), proposal_height);
        let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
            chain_id_hash,
            epoch,
            proposal.clone(),
            vec![entrypoint.clone()],
            vec![reservation],
            vec![routing_plan],
            vec![None],
            producer,
            producer_keypair.private_key(),
        )
        .expect("canonical autonomous QueuePlan fixture payload");

        let validator_pops = validator_set
            .iter()
            .map(|validator| {
                let keypair = activation_validator_keypairs
                    .iter()
                    .find(|keypair| keypair.public_key() == validator.public_key())
                    .expect("fixture retains every lane validator key");
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("fixture lane validator PoP")
            })
            .collect::<Vec<_>>();
        let selected_keypairs = validator_set
            .iter()
            .take(usize::try_from(min_quorum).expect("fixture quorum fits usize"))
            .map(|validator| {
                activation_validator_keypairs
                    .iter()
                    .find(|keypair| keypair.public_key() == validator.public_key())
                    .expect("fixture retains every selected lane validator key")
            })
            .collect::<Vec<_>>();
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let availability_body = crate::lane_consensus::lane_payload_availability_body(
            &payload,
            &proposal,
            chain_id_hash,
            epoch,
        )
        .expect("fixture availability body");
        let prepare_votes = selected_keypairs
            .iter()
            .map(|keypair| {
                let availability_vote =
                    crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                        availability_body.clone(),
                        PeerId::new(keypair.public_key().clone()),
                        validator_pops.clone(),
                        keypair.private_key(),
                    )
                    .expect("fixture availability vote");
                crate::lane_consensus::LaneBlockVoteV1 {
                    body: prepare_body.clone(),
                    signer: PeerId::new(keypair.public_key().clone()),
                    bls_signature: Signature::try_new(
                        keypair.private_key(),
                        &prepare_body.signature_preimage(),
                    )
                    .expect("fixture prepare signature")
                    .payload()
                    .to_vec(),
                    payload_availability_vote: Some(availability_vote),
                }
            })
            .collect::<Vec<_>>();
        let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &prepare_votes,
        )
        .expect("fixture availability-certified PrepareQC");
        let commit_votes = selected_keypairs
            .iter()
            .map(|keypair| {
                signed_lane_block_vote_for_state_test(&proposal, CertPhase::Commit, keypair)
            })
            .collect::<Vec<_>>();
        let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            proposal.vote_body(CertPhase::Commit),
            validator_set,
            &commit_votes,
        )
        .expect("fixture CommitQC");
        let signer_pops = selected_keypairs
            .iter()
            .map(|keypair| {
                (
                    keypair.public_key().clone(),
                    iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                        .expect("fixture selected signer PoP"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let certified = crate::kura::CertifiedLaneBlockArtifact::new(
            crate::lane_consensus::CommittedLaneBlockSession {
                proposal: proposal.clone(),
                prepare_qc: prepare_qc.clone(),
                commit_qc,
            },
            signer_pops,
        );
        let autonomous = crate::kura::AutonomousLaneBlockArtifact {
            format: crate::kura::AutonomousLaneBlockArtifactFormat::Current,
            executable_payload: payload.clone(),
            availability_certificate: Some(
                crate::lane_consensus::DurableLanePayloadAvailabilityCertificateV1 {
                    certificate: prepare_qc,
                },
            ),
            view_checkpoint: None,
            new_view_certificates: Vec::new(),
        };
        let bundle = crate::kura::AutonomousLaneMergeBundleV1 {
            version: crate::kura::AutonomousLaneMergeBundleV1::VERSION,
            autonomous,
            certified: certified.clone(),
        };
        let source_bundle = bundle
            .encode_framed()
            .expect("fixture autonomous bundle encoding");
        crate::kura::Kura::validate_autonomous_lane_merge_bundle(&bundle, chain_id_hash, epoch)
            .expect("fixture autonomous bundle validation");
        let (proposal_block_hash, proposal_view) =
            crate::kura::Kura::autonomous_lane_execution_anchor(&proposal, payload.payload_hash);
        let descriptor = &proposal.descriptor;
        let ownership = SumeragiLanePayloadOwnership {
            proposal_height,
            proposal_view,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            lane_block_height,
            lane_block_view,
            subject_hash: descriptor.subject_hash,
            qc_mode_tag: descriptor.qc_mode_tag.clone(),
            accepted_candidate_indices: descriptor.accepted_candidate_indices.clone(),
            accepted_transaction_hashes: descriptor.accepted_transaction_hashes.clone(),
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Some(descriptor.descriptor_hash),
            lane_block_descriptor_validator_set: descriptor.validator_set.clone(),
            lane_block_descriptor_validator_count: descriptor.validator_count,
            lane_block_descriptor_min_quorum: descriptor.min_quorum,
            payload_ownership_hash: descriptor.payload_ownership_hash,
            rbc_instance_hash: descriptor.rbc_instance_hash,
        };
        let input = crate::kura::LaneBlockExecutionInputArtifact::new(
            crate::kura::RecoveredLaneBlockPayload {
                proposal: proposal.clone(),
                artifact: crate::kura::LaneBlockArtifact::new(proposal_block_hash, ownership),
                autonomous_chain_id_hash: Some(chain_id_hash),
                autonomous_epoch: Some(epoch),
                autonomous_payload_hash: Some(payload.payload_hash),
                entrypoints: vec![entrypoint],
                reservation_keys: payload.reservation_keys.clone(),
                routing_plans: payload.routing_plans.clone(),
                native_amx_receipts: payload.native_amx_receipts.clone(),
            },
        );
        MergeExecutionSource {
            bundle_hash: merge_execution_source_bundle_hash(&source_bundle),
            source_bundle,
            origin_proposal: proposal,
            certified,
            input,
        }
    }

    fn persist_merge_carrier_finality_chain_for_state_test(
        state: &State,
        parent: &SignedBlock,
        carrier: &SignedBlock,
        keypairs: &[KeyPair],
    ) {
        use iroha_data_model::block::consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PayloadEncoding, PROTOCOL_VERSION,
            QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
        };

        fn artifact_for_block(
            state: &State,
            block: &SignedBlock,
            parent: Option<&V2FinalityArtifact>,
            keypairs: &[KeyPair],
        ) -> V2FinalityArtifact {
            assert!(!keypairs.is_empty(), "finality fixture requires validators");
            let mut keypairs = keypairs.iter().collect::<Vec<_>>();
            keypairs.sort_by_key(|keypair| PeerId::new(keypair.public_key().clone()));
            let roster = keypairs
                .iter()
                .map(|keypair| ValidatorPower {
                    validator: PeerId::new(keypair.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let height = block.header().height().get();
            assert_eq!(
                parent.map_or(1, |artifact| artifact.height.saturating_add(1)),
                height,
                "fixture finality must form one contiguous chain",
            );
            let context = HeightContext {
                chain_id: state.chain_id.clone(),
                protocol_version: PROTOCOL_VERSION,
                height,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: ConsensusMode::Permissioned,
                parent_commit_qc: parent.map(|artifact| artifact.commit_qc.clone()),
                snapshot_bootstrap: None,
                quorum: DualQuorum::from_roster(&roster).expect("valid finality quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"state merge finality nexus context"),
                execution_policy_hash: Hash::new(b"state merge finality execution policy"),
                da_layout: DataAvailabilityLayout {
                    encoding: PayloadEncoding::Plain,
                    chunk_size_bytes: 1_024,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 4_096,
                    max_chunk_count: 4,
                },
                leader_seed: [0x42; 32],
            };
            let executed_block_wire = block.encode_wire().expect("canonical executed block wire");
            let height_bytes = height.to_le_bytes();
            let mut execution_commitment = ExecutionCommitment::new_without_merge_carrier(
                Hash::new_from_chunks(&[
                    b"state merge finality parent state".as_slice(),
                    height_bytes.as_slice(),
                ]),
                Hash::new_from_chunks(&[
                    b"state merge finality post state".as_slice(),
                    height_bytes.as_slice(),
                ]),
                Hash::new_from_chunks(&[
                    b"state merge finality ordinary writes".as_slice(),
                    height_bytes.as_slice(),
                ]),
                None,
                0,
                u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64"),
                Hash::new(&executed_block_wire),
            )
            .expect("canonical finality execution commitment");
            execution_commitment.merge_carrier = block
                .execution_context()
                .and_then(|context| context.merge_entry.as_ref())
                .map(|reference| {
                    iroha_data_model::block::consensus_v2::MergeCarrierCommitmentV1::new(
                        reference.entry_hash,
                    )
                });
            let subject = BlockSubject {
                parent_block_hash: block.header().prev_block_hash(),
                block_hash: block.hash(),
                payload_hash: block
                    .canonical_proposal_wire_hash()
                    .expect("canonical proposal block wire"),
            };
            let round = ConsensusRound {
                context_id: context.id(),
                height,
                view: block.header().view_change_index(),
            };
            let signers = (0..keypairs.len())
                .map(|index| u32::try_from(index).expect("fixture signer index fits u32"))
                .collect::<Vec<_>>();
            let mut commit_qc = QuorumCertificate {
                round,
                proposal_round: round,
                phase: GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers,
                aggregate_signature: vec![1],
            };
            let preimage = commit_qc
                .signer_preimage(&context, 0)
                .expect("valid finality signer preimage");
            let signatures = keypairs
                .iter()
                .map(|keypair| {
                    Signature::try_new(keypair.private_key(), &preimage)
                        .expect("sign finality fixture vote")
                        .payload()
                        .to_vec()
                })
                .collect::<Vec<_>>();
            let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
            commit_qc.aggregate_signature =
                iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                    .expect("aggregate finality fixture votes");
            let validator_set_pops = keypairs
                .iter()
                .map(|keypair| {
                    iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                        .expect("derive finality fixture proof of possession")
                })
                .collect();
            let artifact =
                V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
            artifact.verify().expect("fixture finality verifies");
            artifact
        }

        let parent_finality = artifact_for_block(state, parent, None, keypairs);
        let _ = state
            .kura
            .store_v2_finality_artifact(&parent_finality)
            .expect("persist exact parent finality");
        let carrier_finality =
            artifact_for_block(state, carrier, Some(&parent_finality), keypairs);
        let _ = state
            .kura
            .store_v2_finality_artifact(&carrier_finality)
            .expect("persist exact merge-carrier finality");
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the fixture builds one fully certified autonomous execution carrier"
    )]
    fn autonomous_merge_commit_authorization_fixture(
        seed_expired_axt_replay: bool,
        seed_due_start_effect: bool,
    ) -> (
        State,
        MergeLedgerEntry,
        SignedBlock,
        Option<AxtHandleReplayKey>,
    ) {
        let (state, validator_keypairs, commit_keypairs, parent) =
            configured_single_lane_merge_state();
        if seed_due_start_effect {
            let mut locks = GovernanceLocksForReferendum::default();
            locks.locks.insert(
                (*ALICE_ID).clone(),
                GovernanceLockRecord {
                    owner: (*ALICE_ID).clone(),
                    amount: Quantity::from(1_u32),
                    slashed: Quantity::zero(),
                    expiry_height: 1,
                    direction: 0,
                    duration_blocks: 0,
                    custody: Some(GovernanceLockCustody {
                        escrowed: false,
                        asset_definition_id: state.gov.voting_asset_id.clone(),
                        bond_escrow_account: state.gov.bond_escrow_account.clone(),
                        slash_receiver_account: state.gov.slash_receiver_account.clone(),
                    }),
                },
            );
            let mut world = state.world.block();
            world
                .governance_locks
                .insert("autonomous-merge-due-start-effect".to_owned(), locks);
            world.commit();
        }
        let expired_axt_replay_key = seed_expired_axt_replay.then(|| {
            let key = AxtHandleReplayKey::from_parts([0xA7; 32], 1, 1, LaneId::SINGLE);
            let mut replay = state.world.axt_replay_ledger.block();
            replay.insert(
                key,
                AxtReplayRecord {
                    dataspace: DataSpaceId::UNIVERSAL,
                    used_slot: 0,
                    retain_until_slot: 0,
                },
            );
            replay.commit();
            key
        });

        let tag = 0x6A;
        let entrypoint = queue_plan_entrypoint_for_state_test(&state, tag);
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (binding, _) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan.clone(),
            &validator_keypairs,
            1,
            tag,
        );
        {
            let mut world = state.world.block();
            world.accounts.insert(
                entrypoint.authority().clone(),
                AccountValue::new(AccountDetails::default()),
            );
            world.smart_contract_state.insert(
                State::queue_plan_admission_registry_marker_key(&binding.registry_key())
                    .expect("fixture registry key"),
                State::queue_plan_admission_registry_marker_payload(&binding.registry_value())
                    .expect("fixture registry value"),
            );
            world.commit();
        }
        let source = autonomous_merge_source_for_queue_plan_admission_test(
            &state,
            &binding,
            entrypoint,
            routing_plan,
            &commit_keypairs,
        );
        let application_header = BlockHeader::new(
            nonzero!(2_u64),
            Some(parent.hash()),
            None,
            None,
            u64::try_from(parent.header().creation_time().as_millis())
                .expect("fixture parent time fits u64")
                .saturating_add(1),
            0,
        );
        let batch = state
            .build_merge_execution_batch_from_source_prefix(1, application_header, vec![source])
            .expect("fixture source produces a canonical autonomous execution batch");
        let lifecycle = state.lane_consensus_lifecycle_snapshot();
        let active_lanes = lifecycle
            .nexus
            .lane_catalog
            .lanes()
            .iter()
            .map(|lane| MergeLaneBinding {
                lane_id: lane.id,
                dataspace_id: lane.dataspace_id,
                lane_config_hash: merge_lane_config_hash(lane),
                incarnation: lifecycle.incarnations[&lane.id],
                activation_height: lifecycle.activation_heights[&lane.id].saturating_add(1),
            })
            .collect::<Vec<_>>();
        let incarnation_entries = active_lanes
            .iter()
            .map(
                |lane| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                    lane_id: lane.lane_id,
                    incarnation: lane.incarnation,
                },
            )
            .collect::<Vec<_>>();
        let candidate = crate::merge::MergeLedgerCandidate {
            version: crate::merge::MergeLedgerCandidate::VERSION,
            epoch_id: 1,
            view: 0,
            carrier_height: 2,
            carrier_parent_hash: parent.hash(),
            lane_catalog_hash: merge_lane_catalog_hash(&lifecycle.nexus.lane_catalog),
            incarnation_root: LaneLifecycleParameterV1::incarnation_root(&incarnation_entries),
            activation_root: crate::merge::merge_activation_root(&active_lanes),
            active_lanes,
            lane_snapshots: Vec::new(),
            execution_batch: Some(batch),
            lane_drain_certificates: Vec::new(),
            queue_plan_admissions: Vec::new(),
            global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
        };
        state
            .validate_merge_candidate_for_global_round(&candidate, &parent.header(), 0)
            .expect("fixture autonomous execution candidate is valid");
        let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
        let entry = merge_entry_from_candidate(candidate, qc);
        let carrier = certified_merge_carrier_after(&parent, &entry);
        state
            .kura
            .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
            .expect("persist exact autonomous execution carrier");
        persist_merge_carrier_finality_chain_for_state_test(
            &state,
            &parent,
            &carrier,
            &commit_keypairs,
        );
        (state, entry, carrier, expired_axt_replay_key)
    }

    fn staged_autonomous_merge_commit_block<'state>(
        state: &'state State,
        entry: &MergeLedgerEntry,
        carrier: &SignedBlock,
    ) -> StateBlock<'state> {
        let mut state_block = state
            .block_with_certified_merge_entry(carrier.header().clone(), entry)
            .expect("certified autonomous execution must stage on its exact carrier");
        assert!(
            state_block
                .canonical_wsv_merge_commit_authorization
                .is_some(),
            "successful re-execution must mint canonical WSV commit authorization"
        );
        state_block
            .validate_staged_merge_execution_authorization()
            .expect("pre-vote authorization must match after deterministic start effects");
        let committed = ValidBlock::new_unverified_for_tests(carrier.clone())
            .commit_unchecked()
            .unpack(|_| {});
        let topology = state.commit_topology_snapshot();
        let _events = state_block.apply_without_execution(&committed, topology);
        assert!(
            state_block
                .canonical_carrier_commit_metadata_authorization
                .is_some(),
            "exact finalized carrier application must mint metadata authorization"
        );
        state_block
    }

    #[test]
    fn canonical_wsv_authorization_commits_exact_autonomous_execution_once() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        staged_autonomous_merge_commit_block(&state, &entry, &carrier)
            .commit()
            .expect("exact authorized autonomous execution must commit");

        assert_eq!(state.committed_height(), 2);
        assert!(
            state
                .merge_execution_already_applied(
                    &entry,
                    entry
                        .execution_batch
                        .as_ref()
                        .expect("fixture carries execution"),
                )
                .expect("committed marker lookup"),
            "canonical commit must publish its replay markers"
        );
    }

    #[test]
    fn autonomous_execution_commit_rejects_missing_wsv_authorization() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
        let _authorization = state_block
            .canonical_wsv_merge_commit_authorization
            .take()
            .expect("fixture authorization");

        assert!(matches!(
            state_block.commit(),
            Err(TransactionsBlockError::MergeAdmission)
        ));
        assert_eq!(state.committed_height(), 1);
    }

    #[test]
    fn autonomous_execution_commit_rejects_missing_carrier_metadata_authorization() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
        let _authorization = state_block
            .canonical_carrier_commit_metadata_authorization
            .take()
            .expect("fixture carrier metadata authorization");

        assert!(matches!(
            state_block.commit(),
            Err(TransactionsBlockError::MergeAdmission)
        ));
        assert_eq!(state.committed_height(), 1);
    }

    #[test]
    fn autonomous_execution_commit_rejects_mismatched_wsv_authorization() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
        state_block
            .canonical_wsv_merge_commit_authorization
            .as_mut()
            .expect("fixture authorization")
            .batch_hash = Hash::new(b"mismatched-canonical-wsv-authorization");

        assert!(matches!(
            state_block.commit(),
            Err(TransactionsBlockError::MergeAdmission)
        ));
        assert_eq!(state.committed_height(), 1);
    }

    #[test]
    fn autonomous_execution_commit_rejects_replayed_carrier_metadata_authorization() {
        let (first_state, first_entry, first_carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut first_block =
            staged_autonomous_merge_commit_block(&first_state, &first_entry, &first_carrier);
        let replayed_authorization = first_block
            .canonical_carrier_commit_metadata_authorization
            .take()
            .expect("first fixture carrier metadata authorization");
        drop(first_block);

        let (second_state, second_entry, second_carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut second_block =
            staged_autonomous_merge_commit_block(&second_state, &second_entry, &second_carrier);
        second_block.canonical_carrier_commit_metadata_authorization = Some(replayed_authorization);

        assert!(matches!(
            second_block.commit(),
            Err(TransactionsBlockError::MergeAdmission)
        ));
        assert_eq!(second_state.committed_height(), 1);
    }

    #[test]
    fn autonomous_execution_commit_rejects_stale_authorized_base() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
        state_block
            .canonical_wsv_merge_commit_authorization
            .as_mut()
            .expect("fixture authorization")
            .base_state_height = 0;

        assert!(matches!(
            state_block.commit(),
            Err(TransactionsBlockError::MergeAdmission)
        ));
        assert_eq!(state.committed_height(), 1);
    }

    #[test]
    fn autonomous_execution_commit_rejects_post_stage_wsv_drift() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
        let drift_key = StatePath::from_str("canonical_wsv_authorization_post_stage_drift")
            .expect("fixture state path");
        state_block
            .world
            .smart_contract_state
            .insert(drift_key, vec![0xD1]);

        assert!(matches!(
            state_block.commit(),
            Err(TransactionsBlockError::MergeAdmission)
        ));
        assert_eq!(state.committed_height(), 1);
    }

    #[test]
    fn autonomous_execution_commit_rejects_post_stage_runtime_surface_drift() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
        let peer = PeerId::new(
            checked_keypair_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        state_block
            .commit_topology
            .mutate_vec(|topology| topology.push(peer));

        assert!(matches!(
            state_block.commit(),
            Err(TransactionsBlockError::MergeAdmission)
        ));
        assert_eq!(state.committed_height(), 1);
    }

    #[test]
    fn autonomous_execution_defers_expired_axt_replay_pruning() {
        let (state, entry, carrier, expired_key) =
            autonomous_merge_commit_authorization_fixture(true, false);
        let expired_key = expired_key.expect("fixture expired replay key");
        staged_autonomous_merge_commit_block(&state, &entry, &carrier)
            .commit()
            .expect("authorized execution carrier must not gain AXT pruning effects");

        assert!(
            state
                .world
                .axt_replay_ledger
                .view()
                .get(&expired_key)
                .is_some(),
            "expired replay guards must remain for a later non-execution carrier"
        );
    }

    #[test]
    fn autonomous_execution_rejects_post_stage_axt_replay_drift() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = staged_autonomous_merge_commit_block(&state, &entry, &carrier);
        state_block.world.axt_replay_ledger.insert(
            AxtHandleReplayKey::from_parts([0xD2; 32], 1, 2, LaneId::SINGLE),
            AxtReplayRecord {
                dataspace: DataSpaceId::UNIVERSAL,
                used_slot: 0,
                retain_until_slot: 0,
            },
        );

        assert!(matches!(
            state_block.commit(),
            Err(TransactionsBlockError::MergeAdmission)
        ));
        assert_eq!(state.committed_height(), 1);
    }

    #[test]
    fn autonomous_execution_stage_rejects_preexisting_axt_replay_overlay() {
        let (state, entry, carrier, _) =
            autonomous_merge_commit_authorization_fixture(false, false);
        let mut state_block = state.lane_application_block(carrier.header().clone());
        state_block.world.axt_replay_ledger.insert(
            AxtHandleReplayKey::from_parts([0xD3; 32], 1, 3, LaneId::SINGLE),
            AxtReplayRecord {
                dataspace: DataSpaceId::UNIVERSAL,
                used_slot: 0,
                retain_until_slot: 0,
            },
        );

        assert!(matches!(
            state_block.stage_certified_merge_entry(&entry),
            Err(MergeLedgerCommitError::ExecutionStageNotPristine)
        ));
    }

    #[test]
    fn autonomous_execution_pre_vote_rejects_due_start_of_block_effect() {
        let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture(false, true);
        let state_block = state
            .block_with_certified_merge_entry(carrier.header().clone(), &entry)
            .expect("certified execution stages before due block effects run");

        assert!(matches!(
            state_block.validate_staged_merge_execution_authorization(),
            Err(MergeLedgerCommitError::ExecutionDivergence(_))
                | Err(MergeLedgerCommitError::ExecutionBatchInvalid(_))
        ));
        assert!(
            state_block
                .world
                .governance_locks
                .get("autonomous-merge-due-start-effect")
                .is_some_and(|locks| locks.locks.is_empty()),
            "the regression fixture must exercise an actual due start-of-block mutation"
        );
    }

    fn configured_two_lane_merge_state() -> (State, Vec<KeyPair>, Vec<KeyPair>, SignedBlock) {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
        let lane_one = LaneConfig {
            id: LaneId::new(1),
            alias: "replaceable-lane".to_owned(),
            ..LaneConfig::default()
        };
        let lane_catalog = LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), lane_one])
            .expect("two-lane merge fixture catalog");
        state
            .set_nexus(iroha_config::parameters::actual::Nexus {
                enabled: true,
                lane_catalog,
                ..iroha_config::parameters::actual::Nexus::default()
            })
            .expect("enable two-lane Nexus merge fixture");

        let (validator_ids, validator_keypairs) = bls_accounts_in("validators", 4);
        seed_consensus_keys_with_pops(&state, &validator_keypairs);
        install_lane_manifest_registry(
            &state,
            &[
                (
                    LaneId::SINGLE,
                    DataSpaceId::UNIVERSAL,
                    validator_ids.clone(),
                ),
                (LaneId::new(1), DataSpaceId::UNIVERSAL, validator_ids),
            ],
        );
        let commit_keypairs = configure_commit_topology_preserving_world_peers(&state, 1);

        let parent = empty_global_block_after(None);
        kura.store_block(Arc::new(parent.clone()))
            .expect("store two-lane merge fixture carrier parent");
        commit_block_metadata_to_state(&state, &parent);
        (state, validator_keypairs, commit_keypairs, parent)
    }

    #[test]
    fn equivalent_queue_plan_quorums_collapse_to_one_deterministic_admission() {
        let (state, validator_keypairs, _, parent) = configured_single_lane_merge_state();
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (binding, first_certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            1,
            0x50,
        );
        let coordinator = &binding.admission_context.route_incarnations[0];
        assert_eq!(coordinator.durability_threshold, 2);
        let alternate_attestations = [2_usize, 3]
            .into_iter()
            .map(|index| {
                let validator = &coordinator.validator_set[index];
                let keypair = validator_keypairs
                    .iter()
                    .find(|keypair| keypair.public_key() == validator.public_key())
                    .expect("fixture retains alternate quorum signer");
                let validator_index = u16::try_from(index).expect("validator index fits u16");
                let signing_bytes =
                    crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v2(
                        binding.canonical_hash(),
                        validator_index,
                    )
                    .expect("alternate quorum signing bytes");
                crate::torii_proxy::QueuePlanAdmissionAttestationV2 {
                    version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2,
                    validator_index,
                    signature: Signature::try_new(keypair.private_key(), &signing_bytes)
                        .expect("alternate quorum signature"),
                }
            })
            .collect();
        let alternate_certificate =
            norito::to_bytes(&crate::torii_proxy::QueuePlanAdmissionCertificateV2 {
                version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
                binding,
                attestations: alternate_attestations,
            })
            .expect("alternate quorum certificate");
        assert_ne!(first_certificate, alternate_certificate);

        let expected = first_certificate.clone().min(alternate_certificate.clone());
        let forward = state
            .merge_candidate_with_queue_plan_admissions(
                &parent.header(),
                0,
                None,
                vec![first_certificate.clone(), alternate_certificate.clone()],
            )
            .expect("equivalent quorum certificates are idempotent")
            .expect("QueuePlan controls produce a candidate");
        let reverse = state
            .merge_candidate_with_queue_plan_admissions(
                &parent.header(),
                0,
                None,
                vec![alternate_certificate, first_certificate],
            )
            .expect("input order does not affect equivalent quorum collapse")
            .expect("QueuePlan controls produce a candidate");

        assert_eq!(forward.queue_plan_admissions, vec![expected]);
        assert_eq!(forward.canonical_bytes(), reverse.canonical_bytes());
    }

    #[test]
    fn queue_plan_only_carriers_require_exact_committed_active_lane_bindings() {
        let (state, validator_keypairs, commit_keypairs, parent) =
            configured_two_lane_merge_state();
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (_, certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            1,
            0x51,
        );
        let candidate = state
            .merge_candidate_with_queue_plan_admissions(
                &parent.header(),
                0,
                None,
                vec![certificate],
            )
            .expect("canonical QueuePlan candidate construction")
            .expect("QueuePlan controls produce a standalone candidate");
        assert!(candidate.execution_batch.is_none());
        assert!(candidate.lane_snapshots.is_empty());
        assert!(candidate.lane_drain_certificates.is_empty());

        let mut stale_incarnation = candidate.clone();
        stale_incarnation.active_lanes[1].incarnation =
            Hash::new(b"forged-unrelated-lane-incarnation");
        let incarnation_entries = stale_incarnation
            .active_lanes
            .iter()
            .map(
                |binding| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                    lane_id: binding.lane_id,
                    incarnation: binding.incarnation,
                },
            )
            .collect::<Vec<_>>();
        stale_incarnation.incarnation_root =
            iroha_data_model::nexus::LaneLifecycleParameterV1::incarnation_root(
                &incarnation_entries,
            );
        stale_incarnation.activation_root =
            crate::merge::merge_activation_root(&stale_incarnation.active_lanes);
        assert!(matches!(
            state.validate_merge_candidate_for_global_round(
                &stale_incarnation,
                &parent.header(),
                0,
            ),
            Err(MergeLedgerCommitError::IncarnationContext(_))
        ));

        let mut forged_config = candidate;
        forged_config.active_lanes[1].lane_config_hash = Hash::new(b"forged-unrelated-lane-config");
        let forged_qc = merge_qc_for_candidate(&state, &forged_config, &commit_keypairs, &[0]);
        let forged_entry = merge_entry_from_candidate(forged_config, forged_qc);
        assert!(matches!(
            state.validate_certified_merge_entry_for_global_order(&forged_entry),
            Err(MergeLedgerCommitError::IncarnationContext(_))
        ));
    }

    #[test]
    fn restart_authenticates_queue_plan_predecessor_from_durable_kura_before_state_replay() {
        let (state, validator_keypairs, commit_keypairs, parent) =
            configured_single_lane_merge_state();
        let kura = Arc::clone(&state.kura);
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (_, certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            1,
            0x52,
        );
        let candidate = state
            .merge_candidate_with_queue_plan_admissions(
                &parent.header(),
                0,
                None,
                vec![certificate],
            )
            .expect("canonical QueuePlan candidate construction")
            .expect("QueuePlan controls produce a standalone candidate");
        let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
        let entry = merge_entry_from_candidate(candidate, qc);
        let carrier = certified_merge_carrier_after(&parent, &entry);
        kura.store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
            .expect("persist QueuePlan merge carrier before State publication");
        drop(state);

        let mut restarted = State::try_new_with_chain(
            World::default(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            (*DEFAULT_TEST_CHAIN_ID).clone(),
            #[cfg(feature = "telemetry")]
            <_>::default(),
        )
        .expect("durable QueuePlan predecessor authenticates before State block replay");
        assert_eq!(
            restarted.committed_height(),
            0,
            "startup recovery must not pretend future Kura carriers are in State"
        );
        assert!(
            restarted.merge_ledger().is_empty(),
            "the future carrier remains unpublished until exact State replay"
        );

        restarted.push_block_hash_for_testing(parent.hash());
        restarted.push_block_hash_for_testing(carrier.hash());
        restarted
            .recover_merge_ledger_from_kura()
            .expect("exact State replay hydrates the authenticated QueuePlan carrier");
        assert_eq!(restarted.merge_ledger().snapshot()[0].as_ref(), &entry);
    }

    #[test]
    fn restart_rejects_queue_plan_predecessor_conflicting_with_durable_kura() {
        let (state, validator_keypairs, commit_keypairs, parent) =
            configured_single_lane_merge_state();
        let kura = Arc::clone(&state.kura);
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (_, valid_certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan.clone(),
            &validator_keypairs,
            1,
            0x53,
        );
        let mut candidate = state
            .merge_candidate_with_queue_plan_admissions(
                &parent.header(),
                0,
                None,
                vec![valid_certificate],
            )
            .expect("canonical QueuePlan candidate construction")
            .expect("QueuePlan controls produce a standalone candidate");

        let conflicting_predecessor = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"conflicting-queue-plan-restart-predecessor",
        ));
        {
            let mut block_hashes = state.block_hashes.block_and_revert();
            block_hashes.push(conflicting_predecessor);
            block_hashes.commit();
        }
        let (_, conflicting_certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            1,
            0x54,
        );
        {
            let mut block_hashes = state.block_hashes.block_and_revert();
            block_hashes.push(parent.hash());
            block_hashes.commit();
        }
        candidate.queue_plan_admissions = vec![conflicting_certificate];
        let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
        let entry = merge_entry_from_candidate(candidate, qc);
        let carrier = certified_merge_carrier_after(&parent, &entry);
        kura.store_block_with_merge_entry(Arc::new(carrier), &entry)
            .expect("persist cryptographically valid conflicting QueuePlan carrier");
        drop(state);

        let recovery = State::try_new_with_chain(
            World::default(),
            kura,
            LiveQueryStore::start_test(),
            (*DEFAULT_TEST_CHAIN_ID).clone(),
            #[cfg(feature = "telemetry")]
            <_>::default(),
        );
        let error = match recovery {
            Ok(_) => panic!("durable Kura history must reject a conflicting QueuePlan predecessor"),
            Err(error) => error,
        };
        assert!(
            matches!(
                error,
                MergeLedgerCommitError::ExecutionBatchInvalid(ref message)
                    if message.contains(
                        "queue-plan admission predecessor is absent or differs from canonical history"
                    )
            ),
            "unexpected conflicting predecessor rejection: {error}"
        );
    }

    #[test]
    fn queue_plan_registry_staging_is_an_exact_idempotent_compare_and_set() {
        let (state, validator_keypairs, _, parent) = configured_two_lane_merge_state();
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            1,
            0x59,
        );
        assert_eq!(
            state
                .queue_plan_admission_binding_registry_match(&binding)
                .expect("absent registry lookup"),
            QueuePlanAdmissionRegistryMatch::Absent
        );
        let candidate = state
            .merge_candidate_with_queue_plan_admissions(
                &parent.header(),
                0,
                None,
                vec![certificate.clone()],
            )
            .expect("canonical QueuePlan candidate")
            .expect("standalone QueuePlan candidate");
        let carrier = empty_global_block_after(Some(&parent));
        let mut state_block = state.block(carrier.header());
        let write_set_before = state_block.merge_execution_write_set_root();
        state_block
            .stage_queue_plan_admissions(&[certificate.clone()], &candidate.active_lanes, 2)
            .expect("absent registry marker is inserted");
        let write_set_after_insert = state_block.merge_execution_write_set_root();
        assert_ne!(
            write_set_after_insert, write_set_before,
            "QueuePlan registry writes must enter the signed final WSV write-set commitment"
        );
        state_block
            .stage_queue_plan_admissions(&[certificate.clone()], &candidate.active_lanes, 2)
            .expect("the exact marker is idempotent");
        assert_eq!(
            state_block.merge_execution_write_set_root(),
            write_set_after_insert,
            "idempotent exact CAS replay must not change the committed write set"
        );
        let key = State::queue_plan_admission_registry_marker_key(&binding.registry_key())
            .expect("fixture registry key");
        let expected_payload =
            State::queue_plan_admission_registry_marker_payload(&binding.registry_value())
                .expect("fixture registry value");
        assert_eq!(
            state_block.world.smart_contract_state.get(&key),
            Some(&expected_payload)
        );
        // `StateBlock` owns exclusive MV storage transactions. Release this
        // overlay before opening an independent one for the conflict case.
        drop(state_block);

        let mut conflicting_block = state.block(carrier.header());
        let conflicting_value = crate::torii_proxy::QueuePlanAdmissionRegistryValueV2 {
            version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2,
            binding_hash: Hash::new(b"conflicting-cas-value"),
        };
        conflicting_block.world.smart_contract_state.insert(
            key,
            State::queue_plan_admission_registry_marker_payload(&conflicting_value)
                .expect("well-formed conflicting registry value"),
        );
        assert!(matches!(
            conflicting_block.stage_queue_plan_admissions(
                &[certificate],
                &candidate.active_lanes,
                2,
            ),
            Err(MergeLedgerCommitError::ExecutionMarkerConflict(_))
        ));
    }

    #[test]
    fn queue_plan_registry_presence_is_bounded_and_malformed_markers_fail_closed() {
        let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (binding, _) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            1,
            0x5A,
        );
        assert!(
            !state
                .queue_plan_admission_registry_entrypoint_present(binding.entrypoint_hash.clone(),)
                .expect("absent registry presence lookup")
        );

        let key = State::queue_plan_admission_registry_marker_key(&binding.registry_key())
            .expect("fixture registry key");
        let payload =
            State::queue_plan_admission_registry_marker_payload(&binding.registry_value())
                .expect("fixture registry value");
        {
            let mut world = state.world.block();
            world.smart_contract_state.insert(key.clone(), payload);
            world.commit();
        }
        assert!(
            state
                .queue_plan_admission_registry_entrypoint_present(binding.entrypoint_hash.clone(),)
                .expect("well-formed registry presence lookup")
        );

        {
            let mut world = state.world.block();
            world.smart_contract_state.insert(key, vec![0x00]);
            world.commit();
        }
        assert!(
            state
                .queue_plan_admission_registry_entrypoint_present(binding.entrypoint_hash)
                .is_err(),
            "a malformed marker must not be treated as an absent or canonical admission"
        );
    }

    #[test]
    fn pending_queue_plan_admission_uses_legacy_topology_when_nexus_is_disabled() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = State::new_for_testing(
            World::default(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        let mut nexus = state.nexus_snapshot();
        nexus.enabled = false;
        state
            .set_nexus(nexus)
            .expect("apply disabled Nexus state for legacy QueuePlan route");
        let validator_keypairs = configure_commit_topology(&state, 1);

        let parent = empty_global_block_after(None);
        kura.store_block(Arc::new(parent.clone()))
            .expect("store legacy QueuePlan carrier parent");
        commit_block_metadata_to_state(&state, &parent);

        let route = crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        assert!(
            state
                .authoritative_lane_peer_ids_at_height(route.lane_id, 2)
                .is_empty(),
            "disabled Nexus has no lane-registry authority; QueuePlan must use commit topology"
        );
        let routing_plan = crate::queue::RoutingPlan::single(route);
        let (_, certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan,
            &validator_keypairs,
            1,
            0x62,
        );

        assert_eq!(
            state
                .classify_pending_queue_plan_admission(&certificate, 2)
                .expect("legacy QueuePlan certificate is classifiable")
                .1,
            PendingQueuePlanAdmissionDisposition::EligibleAbsent
        );

        let candidate = state
            .merge_candidate_with_queue_plan_admissions(
                &parent.header(),
                0,
                None,
                vec![certificate],
            )
            .expect("legacy QueuePlan candidate construction")
            .expect("legacy QueuePlan controls produce a standalone candidate");
        let qc = merge_qc_for_candidate(&state, &candidate, &validator_keypairs, &[0]);
        let entry = merge_entry_from_candidate(candidate, qc);
        let carrier = certified_merge_carrier_after(&parent, &entry);
        let staged = state
            .block_with_certified_merge_entry(carrier.header().clone(), &entry)
            .expect("QueuePlan-only certified merge entry remains legal with Nexus disabled");
        assert!(
            staged.canonical_wsv_merge_commit_authorization.is_none()
                && staged
                    .canonical_carrier_commit_metadata_authorization
                    .is_none(),
            "control-only merge entries must never mint autonomous WSV or carrier metadata authority"
        );

        let mut non_control_entry = entry;
        non_control_entry.queue_plan_admissions.clear();
        assert!(matches!(
            state.block_with_certified_merge_entry(
                carrier.header().clone(),
                &non_control_entry,
            ),
            Err(MergeLedgerCommitError::ExecutionBatchInvalid(ref message))
                if message.contains("requires Nexus multilane mode")
        ));
    }

    #[test]
    fn same_carrier_queue_plan_certificate_cannot_authorize_autonomous_execution() {
        let (state, validator_keypairs, commit_keypairs, parent) =
            configured_single_lane_merge_state();
        let tag = 0x61;
        let entrypoint = queue_plan_entrypoint_for_state_test(&state, tag);
        let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        ));
        let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
            &state,
            routing_plan.clone(),
            &validator_keypairs,
            1,
            tag,
        );
        binding
            .validate_for_request(&state.chain_id, &entrypoint, &routing_plan)
            .expect("fixture certificate binds the autonomous transaction");
        {
            let mut world = state.world.block();
            world.accounts.insert(
                entrypoint.authority().clone(),
                AccountValue::new(AccountDetails::default()),
            );
            world.commit();
        }
        let source = autonomous_merge_source_for_queue_plan_admission_test(
            &state,
            &binding,
            entrypoint,
            routing_plan,
            &commit_keypairs,
        );
        let application_header = BlockHeader::new(
            nonzero!(2_u64),
            Some(parent.hash()),
            None,
            None,
            u64::try_from(parent.header().creation_time().as_millis())
                .expect("fixture parent time fits u64")
                .saturating_add(1),
            0,
        );
        assert!(
            state
                .build_merge_execution_batch_from_source_prefix(
                    1,
                    application_header.clone(),
                    vec![source.clone()],
                )
                .is_none(),
            "an availability-certified source remains ineligible while its binding is absent from pre-carrier WSV"
        );

        let registry_key = State::queue_plan_admission_registry_marker_key(&binding.registry_key())
            .expect("fixture registry key");
        {
            let mut world = state.world.block();
            world.smart_contract_state.insert(
                registry_key.clone(),
                State::queue_plan_admission_registry_marker_payload(&binding.registry_value())
                    .expect("fixture registry value"),
            );
            world.commit();
        }
        let batch = state
            .build_merge_execution_batch_from_source_prefix(1, application_header, vec![source])
            .expect("the otherwise-identical source is eligible with exact pre-carrier authority");
        let lifecycle = state.lane_consensus_lifecycle_snapshot();
        let active_lanes = lifecycle
            .nexus
            .lane_catalog
            .lanes()
            .iter()
            .map(|lane| MergeLaneBinding {
                lane_id: lane.id,
                dataspace_id: lane.dataspace_id,
                lane_config_hash: merge_lane_config_hash(lane),
                incarnation: lifecycle.incarnations[&lane.id],
                activation_height: lifecycle.activation_heights[&lane.id].saturating_add(1),
            })
            .collect::<Vec<_>>();
        let incarnation_entries = active_lanes
            .iter()
            .map(
                |lane| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                    lane_id: lane.lane_id,
                    incarnation: lane.incarnation,
                },
            )
            .collect::<Vec<_>>();
        let base = crate::merge::MergeLedgerCandidate {
            version: crate::merge::MergeLedgerCandidate::VERSION,
            epoch_id: 1,
            view: 0,
            carrier_height: 2,
            carrier_parent_hash: parent.hash(),
            lane_catalog_hash: merge_lane_catalog_hash(&lifecycle.nexus.lane_catalog),
            active_lanes: active_lanes.clone(),
            incarnation_root: LaneLifecycleParameterV1::incarnation_root(&incarnation_entries),
            activation_root: crate::merge::merge_activation_root(&active_lanes),
            lane_snapshots: Vec::new(),
            execution_batch: Some(batch),
            lane_drain_certificates: Vec::new(),
            queue_plan_admissions: Vec::new(),
            global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
        };
        state
            .validate_merge_candidate_for_global_round(&base, &parent.header(), 0)
            .expect("fixture base candidate is valid with committed pre-carrier authority");

        {
            let mut world = state.world.block();
            world.smart_contract_state.remove(registry_key);
            world.commit();
        }
        assert!(matches!(
            state.merge_candidate_with_queue_plan_admissions(
                &parent.header(),
                0,
                Some(base),
                vec![certificate],
            ),
            Err(MergeLedgerCommitError::ExecutionBatchInvalid(_))
                | Err(MergeLedgerCommitError::ExecutionDivergence(_))
        ));
        assert_eq!(
            state
                .queue_plan_admission_binding_registry_match(&binding)
                .expect("post-negative registry lookup"),
            QueuePlanAdmissionRegistryMatch::Absent,
            "the rejected same-carrier control must not leak an admission marker into WSV"
        );
    }
