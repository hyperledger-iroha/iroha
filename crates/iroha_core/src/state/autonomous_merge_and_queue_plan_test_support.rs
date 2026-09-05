#[expect(
    clippy::too_many_lines,
    reason = "the fixture assembles one complete availability-certified autonomous source"
)]
fn autonomous_merge_source_for_queue_plan_admission_test(
    state: &State,
    binding: &crate::torii_proxy::QueuePlanAdmissionBindingV1,
    entrypoint: TransactionEntrypoint,
    routing_plan: crate::queue::RoutingPlan,
    activation_validator_keypairs: &[KeyPair],
) -> Result<MergeExecutionSource, crate::lane_consensus::LaneAutonomousArtifactError> {
    let coordinator = binding
        .admission_context
        .route_incarnations
        .first()
        .expect("fixture binding has a coordinator");
    let proposal_height = binding.admission_context.proposal_height;
    // Bind the autonomous certificate to the exact lane authority that
    // production validation resolves at this activation height.  The
    // optimizations branch keeps this authority independent from the global
    // commit topology, so deriving it from commit peers would make the
    // fixture certify a committee that production correctly rejects.
    let validator_set = state
        .resolve_lane_committee_at_height(
            LaneAuthorityRoute::new(
                coordinator.leg.route.lane_id,
                coordinator.leg.route.dataspace_id,
            ),
            proposal_height,
        )
        .expect("fixture lane authority resolves at the admission height")
        .validators()
        .to_vec();
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

    let reservation = crate::queue::LaneQueueReservationKeyV1 {
        version: crate::queue::LaneQueueReservationKeyV1::VERSION,
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
        reservation_owner_hash: Hash::new(b"queue-plan-pre-carrier-autonomous-reservation-owner"),
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
    let network_id = state.network_id;
    let epoch = crate::sumeragi::epoch_for_height_from_world(
        &state.world.view(),
        proposal_height,
        ConsensusMode::Permissioned,
    )
    .expect("permissioned fixture epoch");
    let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal.clone(),
        vec![entrypoint.clone()],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        producer,
        producer_keypair.private_key(),
    )?;
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
        &payload, &proposal, network_id, epoch,
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
        .map(|keypair| signed_lane_block_vote_for_state_test(&proposal, CertPhase::Commit, keypair))
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
    crate::kura::Kura::validate_autonomous_lane_merge_bundle(&bundle, network_id, epoch)
        .expect("fixture autonomous bundle validation");
    let input =
        crate::kura::LaneBlockExecutionInputArtifact::new(crate::kura::RecoveredLaneBlockPayload {
            proposal: proposal.clone(),
            source: crate::kura::LaneBlockExecutionSourceV1::autonomous_lane(
                network_id,
                epoch,
                payload.payload_hash,
            ),
            entrypoints: vec![entrypoint],
            reservation_keys: payload.reservation_keys.clone(),
            routing_plans: payload.routing_plans.clone(),
            native_amx_receipts: payload.native_amx_receipts.clone(),
        });
    Ok(MergeExecutionSource {
        bundle_hash: merge_execution_source_bundle_hash(&source_bundle),
        source_bundle,
        origin_proposal: proposal,
        certified,
        input,
    })
}
fn seed_exact_queue_plan_admission_state_for_test(state: &State, certificate: &[u8]) {
    let admission = crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
        &state.network_id,
        certificate,
    )
    .expect("fixture QueuePlan admission certificate");
    let mut world = state.world.block();
    world.smart_contract_state.insert(
        State::queue_plan_admission_registry_marker_key(&admission.registry_key)
            .expect("fixture registry key"),
        State::queue_plan_admission_registry_marker_payload(&admission.registry_value)
            .expect("fixture registry value"),
    );
    State::stage_queue_plan_pending_obligation_in_storage(
        &mut world.smart_contract_state,
        &admission,
    )
    .expect("fixture pending QueuePlan obligation");
    world.commit();
}
fn seed_pending_queue_plan_binding_state_for_test(
    state: &State,
    binding: &crate::torii_proxy::QueuePlanAdmissionBindingV1,
) {
    state
        .install_queue_plan_pending_binding_for_test(binding)
        .expect("fixture pending QueuePlan binding");
}
fn commit_block_metadata_with_genesis_checkpoint_to_state(state: &State, block: &SignedBlock) {
    commit_block_metadata_to_state(state, block);
    let revision = MusubiResolverIndexRevisionV1::default();
    assert_eq!(
        state.world.view().musubi_resolver_index_revision(),
        revision.get()
    );
    let checkpoint = MusubiRegistrySnapshotV1 {
        finalized_height: block.header().height().get(),
        finalized_block_hash: *block.hash().as_ref(),
        index_revision: revision.get(),
    };
    checkpoint
        .validate()
        .expect("valid genesis resolver checkpoint");
    let mut world = state.world.block();
    assert!(
        world
            .musubi_resolver_index_checkpoints
            .insert(revision, checkpoint)
            .is_none(),
        "genesis resolver checkpoint must be absent before fixture bootstrap",
    );
    world.commit();
}
fn queue_plan_pending_obligation_for_test(
    state: &State,
    certificate: &[u8],
) -> QueuePlanPendingObligationV1 {
    let admission = crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v1(
        &state.network_id,
        certificate,
    )
    .expect("fixture QueuePlan admission certificate");
    State::queue_plan_pending_obligation_from_admission(&admission)
        .expect("fixture pending QueuePlan obligation")
}
fn persist_merge_carrier_finality_chain_for_state_test(
    state: &State,
    parent: &SignedBlock,
    carrier: &SignedBlock,
    keypairs: &[KeyPair],
) {
    use iroha_data_model::block::consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
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
        let network_id = *state.network_id_ref();
        let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(network_id, 0, &roster);
        let context = HeightContext {
            network_id,
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
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
            nexus_amx_context_hash: Hash::new(b"state merge finality nexus context"),
            execution_policy_hash: Hash::new(b"state merge finality execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4_096,
                max_chunk_count: 8,
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
        let signer_count =
            crate::sumeragi::network_topology::commit_quorum_from_len(keypairs.len());
        let signers = (0..signer_count)
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
            .take(signer_count)
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
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
        artifact.verify().expect("fixture finality verifies");
        artifact
    }
    let mut parent_finality = None;
    for height in 1..=parent.header().height().get() {
        let height = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .expect("fixture finality height fits usize");
        let block = state
            .kura
            .get_block(height)
            .expect("contiguous fixture parent block is durable");
        let artifact =
            artifact_for_block(state, block.as_ref(), parent_finality.as_ref(), keypairs);
        let _ = state
            .kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist contiguous parent finality");
        parent_finality = Some(artifact);
    }
    let parent_finality = parent_finality.expect("fixture has a genesis finality artifact");
    assert_eq!(
        parent_finality.subject.block_hash,
        parent.hash(),
        "fixture finality chain ends at the exact carrier parent",
    );
    let carrier_finality = artifact_for_block(state, carrier, Some(&parent_finality), keypairs);
    let _ = state
        .kura
        .store_v2_finality_artifact(&carrier_finality)
        .expect("persist exact merge-carrier finality");
}
fn autonomous_merge_commit_authorization_fixture(
    seed_expired_axt_replay: bool,
    seed_due_start_effect: bool,
) -> (
    State,
    MergeLedgerEntry,
    SignedBlock,
    Option<AxtHandleReplayKey>,
) {
    autonomous_merge_commit_authorization_fixture_inner(
        seed_expired_axt_replay,
        seed_due_start_effect,
        None,
        false,
    )
}
fn autonomous_merge_transfer_commit_authorization_fixture() -> (State, MergeLedgerEntry, SignedBlock)
{
    let (state, entry, carrier, _) = autonomous_merge_commit_authorization_fixture_inner(
        false,
        false,
        Some(QueuePlanTransferFixture::Single),
        false,
    );
    (state, entry, carrier)
}
fn autonomous_merge_batch_transfer_commit_authorization_fixture(
    mode: QueuePlanTransferFixture,
) -> (State, MergeLedgerEntry, SignedBlock) {
    assert!(
        matches!(
            mode,
            QueuePlanTransferFixture::AtomicBatch | QueuePlanTransferFixture::IndependentBatch
        ),
        "batch fixture requires batch settlement semantics"
    );
    let (state, entry, carrier, _) =
        autonomous_merge_commit_authorization_fixture_inner(false, false, Some(mode), false);
    (state, entry, carrier)
}
fn autonomous_sealed_reveal_merge_commit_authorization_fixture()
-> (State, MergeLedgerEntry, SignedBlock) {
    let (state, entry, carrier, _) =
        autonomous_merge_commit_authorization_fixture_inner(false, false, None, true);
    (state, entry, carrier)
}
#[derive(Clone, Copy)]
enum QueuePlanTransferFixture {
    Single,
    AtomicBatch,
    IndependentBatch,
}
fn queue_plan_transfer_entrypoint_for_state_test(
    state: &State,
    tag: u8,
    fixture: QueuePlanTransferFixture,
) -> TransactionEntrypoint {
    let transaction_keypair =
        KeyPair::try_from_seed(vec![tag.wrapping_add(0x31); 32], Algorithm::Ed25519)
            .expect("deterministic QueuePlan transfer key");
    let recipient_keypair =
        KeyPair::try_from_seed(vec![tag.wrapping_add(0x71); 32], Algorithm::Ed25519)
            .expect("deterministic QueuePlan recipient key");
    let second_recipient_keypair =
        KeyPair::try_from_seed(vec![tag.wrapping_add(0x91); 32], Algorithm::Ed25519)
            .expect("deterministic QueuePlan second recipient key");
    let authority = AccountId::new(transaction_keypair.public_key().clone());
    let recipient = AccountId::new(recipient_keypair.public_key().clone());
    let second_recipient = AccountId::new(second_recipient_keypair.public_key().clone());
    let domain_id = DomainId::try_new("universal", "universal").expect("fixture domain");
    let definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("fixture asset name"),
    );
    assert_eq!(
        definition_id.canonical_address(),
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
        "fixture transfer asset must be the registered Nexus fee asset"
    );
    let source_asset_id = AssetId::new(definition_id.clone(), authority.clone());
    let registration_header_hash = state
        .latest_block_header_fast()
        .expect("fixture has an authenticated parent header")
        .hash();
    let asset_incarnation = AxtAssetIncarnationV1::derive(
        state.network_id_ref(),
        &definition_id,
        &registration_header_hash,
        &Hash::new(b"queue-plan-transfer-fixture-registration"),
        0,
    );
    {
        let mut world = state.world.block();
        world.domains.insert(
            domain_id.clone(),
            Domain::new(domain_id.clone()).build(&authority),
        );
        world.accounts.insert(
            authority.clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.accounts.insert(
            recipient.clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.accounts.insert(
            second_recipient.clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.asset_definitions.insert(
            definition_id.clone(),
            AssetDefinition::numeric(
                definition_id.clone(),
                "XOR",
                iroha_data_model::asset::AssetBalancePolicy::Global,
                Some(domain_id.clone()),
            )
            .build(&authority),
        );
        world
            .asset_definition_domains
            .insert(definition_id.clone(), domain_id);
        world
            .axt_asset_incarnations
            .insert(definition_id.clone(), asset_incarnation);
        let initial_balance = match fixture {
            QueuePlanTransferFixture::Single => 10_u32,
            QueuePlanTransferFixture::AtomicBatch | QueuePlanTransferFixture::IndependentBatch => {
                20_u32
            }
        };
        let (asset_id, asset_value) =
            Asset::new(source_asset_id.clone(), Quantity::from(initial_balance)).into_key_value();
        world.assets.insert(asset_id, asset_value);
        world.commit();
    }
    let instruction: iroha_data_model::isi::InstructionBox = match fixture {
        QueuePlanTransferFixture::Single => {
            Transfer::asset_quantity(source_asset_id, 3_u32, recipient).into()
        }
        QueuePlanTransferFixture::AtomicBatch | QueuePlanTransferFixture::IndependentBatch => {
            let entries = vec![
                TransferAssetBatchEntry::with_leg_id(
                    "autonomous-batch-leg-a",
                    authority.clone(),
                    recipient,
                    definition_id.clone(),
                    3_u32,
                ),
                TransferAssetBatchEntry::with_leg_id(
                    "autonomous-batch-leg-b",
                    authority.clone(),
                    second_recipient,
                    definition_id,
                    4_u32,
                ),
            ];
            match fixture {
                QueuePlanTransferFixture::AtomicBatch => TransferAssetBatch::new(entries).into(),
                QueuePlanTransferFixture::IndependentBatch => {
                    TransferAssetBatch::independent(entries).into()
                }
                QueuePlanTransferFixture::Single => unreachable!("matched batch fixture"),
            }
        }
    };
    let mut transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .with_admission_intent(
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    );
    transaction.set_creation_time(Duration::from_millis(1));
    transaction.set_ttl(Duration::from_millis(1));
    TransactionEntrypoint::External(transaction.sign(transaction_keypair.private_key()))
}
fn autonomous_merge_commit_authorization_fixture_inner(
    seed_expired_axt_replay: bool,
    seed_due_start_effect: bool,
    transfer_fixture: Option<QueuePlanTransferFixture>,
    wrap_in_sealed_reveal: bool,
) -> (
    State,
    MergeLedgerEntry,
    SignedBlock,
    Option<AxtHandleReplayKey>,
) {
    let (state, validator_keypairs, commit_keypairs, parent) =
        configured_single_lane_queue_plan_state();
    let authority_height = parent.header().height().get();
    let carrier_height = authority_height
        .checked_add(1)
        .expect("fixture carrier height");
    if seed_due_start_effect {
        let mut locks = GovernanceLocksForReferendum::default();
        locks.locks.insert(
            (*ALICE_ID).clone(),
            GovernanceLockRecord {
                owner: (*ALICE_ID).clone(),
                amount: Quantity::from(1_u32),
                slashed: Quantity::zero(),
                expiry_height: authority_height,
                direction: 0,
                duration_blocks: 0,
                custody: GovernanceLockCustody {
                    escrowed: false,
                    asset_definition_id: state.gov.voting_asset_id.clone(),
                    bond_escrow_account: state.gov.bond_escrow_account.clone(),
                    slash_receiver_account: state.gov.slash_receiver_account.clone(),
                },
            },
        );
        let mut world = state.world.block();
        world.put_governance_locks("autonomous-merge-due-start-effect".to_owned(), locks);
        world.commit();
    }
    let expired_axt_replay_key = seed_expired_axt_replay.then(|| {
        let key = AxtHandleReplayKey::from_parts(
            DataSpaceId::UNIVERSAL,
            axt_replay_incarnation_for_test(0xA7),
            [0xA7; 32],
            1,
            1,
            LaneId::SINGLE,
        );
        let mut replay = state.world.axt_replay_ledger.block();
        replay.insert(key, axt_replay_record_for_key(&key, 0, 0));
        replay.commit();
        key
    });
    let tag = 0x6A;
    let entrypoint = match transfer_fixture {
        Some(fixture) => queue_plan_transfer_entrypoint_for_state_test(&state, tag, fixture),
        None => queue_plan_entrypoint_for_state_test(&state, tag),
    };
    let entrypoint = if wrap_in_sealed_reveal {
        let TransactionEntrypoint::External(signed) = entrypoint else {
            panic!("fixture can only seal an external signed transaction")
        };
        let salt = [0xD7; 32];
        let reveal_deadline_height = carrier_height.saturating_add(32);
        let commitment =
            iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
                state.network_id_ref(),
                &signed,
                salt,
                reveal_deadline_height,
            );
        TransactionEntrypoint::SealedReveal(
            iroha_data_model::transaction::signed::SealedTransactionReveal::new(
                commitment, signed, salt,
            ),
        )
    } else {
        entrypoint
    };
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (binding, certificate) = queue_plan_admission_certificate_for_entrypoint_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        authority_height,
        tag,
        &entrypoint,
    );
    {
        let mut world = state.world.block();
        world.accounts.insert(
            entrypoint.authority().clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.commit();
    }
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let source = autonomous_merge_source_for_queue_plan_admission_test(
        &state,
        &binding,
        entrypoint,
        routing_plan,
        &validator_keypairs,
    )
    .expect("canonical autonomous QueuePlan fixture source");
    let application_header = BlockHeader::new(
        NonZeroU64::new(carrier_height).expect("fixture carrier height is non-zero"),
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
        carrier_height,
        carrier_parent_hash: parent.hash(),
        lane_authority_catalog: state
            .merge_active_lane_authority_snapshot(carrier_height)
            .expect("fixture exact lane authority")
            .2,
        lane_catalog_hash: merge_lane_catalog_hash(&lifecycle.nexus.lane_catalog),
        incarnation_root: LaneLifecycleParameterV1::incarnation_root(&incarnation_entries),
        activation_root: crate::merge::merge_activation_root(&active_lanes),
        active_lanes,
        lane_snapshots: Vec::new(),
        execution_batch: Some(batch),
        lane_drain_certificates: Vec::new(),
        global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
    };
    state
        .validate_merge_candidate_for_global_round(
            &candidate,
            &parent.header(),
            0,
            ConsensusMode::Permissioned,
        )
        .expect("fixture autonomous execution candidate is valid");
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let mut carrier = certified_merge_carrier_after(&parent, &entry);
    if let Some(fixture) = transfer_fixture {
        let committed_fragments = match fixture {
            QueuePlanTransferFixture::Single => 1,
            QueuePlanTransferFixture::AtomicBatch | QueuePlanTransferFixture::IndependentBatch => 2,
        };
        carrier.set_committed_fragment_count(committed_fragments);
    }
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist exact autonomous execution carrier");
    persist_merge_carrier_finality_chain_for_state_test(
        &state,
        &parent,
        &carrier,
        &validator_keypairs,
    );
    (state, entry, carrier, expired_axt_replay_key)
}
fn staged_autonomous_merge_commit_block<'state>(
    state: &'state State,
    entry: &MergeLedgerEntry,
    carrier: &SignedBlock,
) -> StateBlock<'state> {
    let mut state_block = state
        .block_with_certified_merge_entry(
            carrier.header().clone(),
            entry,
            ConsensusMode::Permissioned,
        )
        .expect("certified autonomous execution must stage on its exact carrier");
    assert!(
        state_block
            .canonical_wsv_merge_commit_authorization
            .is_some(),
        "successful re-execution must mint canonical WSV commit authorization"
    );
    stage_exact_autonomous_carrier_membership_for_pre_vote(&mut state_block, carrier);
    let (time_entrypoints, time_hashes, time_results) =
        state_block.execute_time_triggers(&carrier.header());
    assert!(time_entrypoints.is_empty());
    assert!(time_hashes.is_empty());
    assert!(time_results.is_empty());
    state_block
        .validate_staged_merge_execution_authorization()
        .expect("pre-vote authorization must bind deterministic carrier events");
    let committed = ValidBlock::new_unverified_for_tests(carrier.clone())
        .commit_unchecked()
        .unpack(|_| {});
    let topology = state.commit_topology_snapshot();
    let (_events, authorization) = state_block.apply_without_execution_inner(
        &committed,
        topology,
        ApplyTopologyAuthority::Fixture,
    );
    authorization.expect("fixture application must authorize the exact canonical carrier");
    assert!(
        state_block
            .canonical_carrier_commit_metadata_authorization
            .is_some(),
        "exact finalized carrier application must mint metadata authorization"
    );
    state_block
}
fn production_validated_autonomous_merge_commit_block<'state>(
    state: &'state State,
    entry: &MergeLedgerEntry,
    carrier: &SignedBlock,
) -> StateBlock<'state> {
    let mut state_block = state
        .block_with_certified_merge_entry(
            carrier.header().clone(),
            entry,
            ConsensusMode::Permissioned,
        )
        .expect("certified autonomous execution must stage on its exact carrier");
    let valid = ValidBlock::validate_unchecked(carrier.clone(), &mut state_block).unpack(|_| {});
    let _witness = state_block
        .take_exec_witness()
        .expect("production validation must hand its execution witness to consensus");
    assert!(
        state_block.batch_transfer_outcomes.is_empty(),
        "production carrier finalization must not inherit autonomous receipt rows"
    );
    let committed = valid.commit_unchecked().unpack(|_| {});
    assert_eq!(
        committed.as_ref().hash(),
        carrier.hash(),
        "production validation must retain the certified carrier identity"
    );
    let topology = state.commit_topology_snapshot();
    let (_events, authorization) = state_block.apply_without_execution_inner(
        &committed,
        topology,
        ApplyTopologyAuthority::Fixture,
    );
    authorization.expect("production-validated carrier application must remain authorized");
    assert!(
        state_block
            .canonical_carrier_commit_metadata_authorization
            .is_some(),
        "production carrier application must mint metadata authorization"
    );
    state_block
        .validate_merge_execution_commit_surface(MergeExecutionCommitSurface::FinalizedCarrier {
            carrier_height: carrier.header().height().get(),
            carrier_hash: &carrier.hash(),
        })
        .expect("production consumer handoff must leave the exact finalized carrier surface");
    state_block
}
fn stage_exact_autonomous_carrier_membership_for_pre_vote(
    state_block: &mut StateBlock<'_>,
    carrier: &SignedBlock,
) {
    let height = autonomous_carrier_transaction_height(state_block);
    state_block
        .stage_canonical_carrier_membership(carrier.entrypoint_hashes(), height)
        .expect("certified carrier membership must match its merge execution batch");
}
fn autonomous_carrier_transaction_height(state_block: &StateBlock<'_>) -> NonZeroUsize {
    usize::try_from(state_block._curr_block.height().get())
        .ok()
        .and_then(NonZeroUsize::new)
        .expect("autonomous carrier height fits canonical transaction storage")
}
fn autonomous_carrier_parent_height(carrier: &SignedBlock) -> usize {
    usize::try_from(
        carrier
            .header()
            .height()
            .get()
            .checked_sub(1)
            .expect("autonomous carrier has a parent"),
    )
    .expect("autonomous carrier parent height fits usize")
}
struct ExactTestStateBlockCommitAuthorization {
    carrier_block_hash: HashOf<BlockHeader>,
    execution_reference: iroha_data_model::block::CertifiedMergeLedgerReference,
    lane_count: usize,
}
impl StateBlockCommitAuthorization for ExactTestStateBlockCommitAuthorization {
    fn consume_for_state_commit(
        self: Box<Self>,
        carrier_block_hash: HashOf<BlockHeader>,
        staged_merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<(), String> {
        let entry = staged_merge_entry
            .filter(|entry| entry.execution_batch.is_some())
            .ok_or_else(|| "test authorization requires one autonomous merge entry".to_owned())?;
        let lane_count = entry
            .execution_batch
            .as_ref()
            .expect("filtered autonomous execution entry")
            .lanes
            .len();
        if carrier_block_hash != self.carrier_block_hash
            || iroha_data_model::block::CertifiedMergeLedgerReference::new(entry)
                != self.execution_reference
            || lane_count != self.lane_count
        {
            return Err("test authorization identity changed before State commit".to_owned());
        }
        Ok(())
    }
}
fn exact_test_state_commit_authorization(
    state_block: &StateBlock<'_>,
) -> Box<dyn StateBlockCommitAuthorization> {
    let entry = state_block
        .staged_merge_entry
        .as_ref()
        .filter(|entry| entry.execution_batch.is_some())
        .expect("fixture State block carries autonomous execution");
    Box::new(ExactTestStateBlockCommitAuthorization {
        carrier_block_hash: state_block._curr_block.hash(),
        execution_reference: iroha_data_model::block::CertifiedMergeLedgerReference::new(entry),
        lane_count: entry
            .execution_batch
            .as_ref()
            .expect("filtered autonomous execution entry")
            .lanes
            .len(),
    })
}
fn commit_staged_autonomous_for_test(
    state_block: StateBlock<'_>,
) -> Result<(), TransactionsBlockError> {
    let authorization = exact_test_state_commit_authorization(&state_block);
    state_block.commit_with_state_commit_authorization(authorization)
}
