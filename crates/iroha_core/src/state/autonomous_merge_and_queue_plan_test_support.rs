#[derive(Clone, Copy)]
enum AutonomousRuntimeEffectFixture {
    Catalog,
    Bootstrap,
}

const AUTONOMOUS_RUNTIME_DATASPACE: &str = "catalogmerge";
const AUTONOMOUS_RUNTIME_LANE: LaneId = LaneId::new(2);

fn configured_runtime_effect_queue_plan_state() -> (State, Vec<KeyPair>, Vec<KeyPair>, SignedBlock)
{
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    nexus.staking.restricted_validator_mode =
        iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
    state
        .set_nexus(nexus.clone())
        .expect("enable native runtime-effect Nexus fixture");
    let (validator_ids, validator_keypairs) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &validator_keypairs);
    let validators = validator_keypairs
        .iter()
        .map(|key| {
            let validator = AccountId::new(key.public_key().clone()).to_string();
            let peer_id = PeerId::new(key.public_key().clone()).to_string();
            norito::json!({ "validator": validator, "peer_id": peer_id })
        })
        .collect::<Vec<_>>();
    let alias = &nexus.lane_catalog.lanes()[0].alias;
    let manifest = norito::json!({
        "lane": alias, "version": 1, "validators": validators, "quorum": 3,
    });
    let directory = tempfile::tempdir().expect("native startup manifest fixture directory");
    std::fs::write(
        directory.path().join(format!("{alias}.manifest.json")),
        norito::json::to_vec(&manifest).expect("native startup manifest JSON"),
    )
    .expect("write public fixture startup manifest");
    let registry_config = iroha_config::parameters::actual::LaneRegistry {
        manifest_directory: Some(directory.path().to_path_buf()),
        ..iroha_config::parameters::actual::LaneRegistry::default()
    };
    let registry = Arc::new(LaneManifestRegistry::from_config(
        &nexus.lane_catalog,
        &nexus.governance,
        &registry_config,
    ));
    assert!(registry.is_bound_to_catalog(&nexus.lane_catalog));
    registry
        .ensure_lane_ready(LaneId::SINGLE)
        .expect("frozen startup lane must be ready");
    let rules = registry
        .lane_rules(LaneId::SINGLE)
        .expect("frozen startup lane has native governance rules");
    assert_eq!(
        rules.validators.iter().cloned().collect::<BTreeSet<_>>(),
        validator_ids.into_iter().collect::<BTreeSet<_>>(),
        "frozen baseline must retain the exact four lane validators"
    );
    assert_eq!(rules.quorum, Some(3));
    state.install_lane_manifests(&registry);
    // The native loader has frozen the source before genesis/checkpoint creation.
    drop(directory);
    let commit_keypairs = configure_commit_topology_preserving_world_peers(&state, 1);
    let parent = empty_global_block_after(None);
    kura.store_block(Arc::new(parent.clone()))
        .expect("store runtime-effect fixture parent");
    commit_block_metadata_with_genesis_checkpoint_to_state(&state, &parent);
    let parent = advance_queue_plan_fixture_to_beacon_parent(&state, parent);
    (state, validator_keypairs, commit_keypairs, parent)
}

fn autonomous_runtime_effect_entrypoint(
    state: &State,
    validator_keypairs: &[KeyPair],
    tag: u8,
    effect: AutonomousRuntimeEffectFixture,
) -> TransactionEntrypoint {
    use iroha_data_model::{
        alias_setup::AliasDataspaceBootstrapGrantV1,
        nexus::{NexusCatalogTransitionV1, RuntimeDataSpaceAdditionV1, RuntimeLaneManifestV1},
    };
    let keypair = KeyPair::try_from_seed(vec![tag.wrapping_add(0x31); 32], Algorithm::Ed25519)
        .expect("deterministic runtime-effect transaction key");
    let authority = AccountId::new(keypair.public_key().clone());
    let grant =
        AliasDataspaceBootstrapGrantV1::try_new(AUTONOMOUS_RUNTIME_DATASPACE, authority.clone())
            .expect("native namespace identity for runtime-effect fixture");
    {
        let mut world = state.world.block();
        world.accounts.insert(
            authority.clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.account_permissions.insert(
            authority.clone(),
            BTreeSet::from([iroha_data_model::permission::Permission::from(
                iroha_executor_data_model::permission::parameter::CanSetParameters,
            )]),
        );
        for key in validator_keypairs {
            world.accounts.insert(
                AccountId::new(key.public_key().clone()),
                AccountValue::new(AccountDetails::default()),
            );
        }
        world.commit();
    }
    let parameter = match effect {
        AutonomousRuntimeEffectFixture::Bootstrap => grant
            .into_custom_parameter()
            .expect("native bootstrap parameter"),
        AutonomousRuntimeEffectFixture::Catalog => {
            let nexus = state.nexus_snapshot();
            let validators = validator_keypairs
                .iter()
                .map(|key| {
                    let validator = AccountId::new(key.public_key().clone()).to_string();
                    let peer_id = PeerId::new(key.public_key().clone()).to_string();
                    norito::json!({
                        "validator": validator,
                        "peer_id": peer_id,
                    })
                })
                .collect::<Vec<_>>();
            NexusCatalogTransitionV1 {
                version: NexusCatalogTransitionV1::VERSION,
                expected_catalog_hash: LaneLifecycleParameterV1::catalog_hash(&nexus.lane_catalog),
                expected_incarnation_root: lane_lifecycle_incarnation_root(
                    &nexus.lane_catalog,
                    &state.lane_incarnations_snapshot(),
                )
                .expect("native fixture incarnation root"),
                expected_runtime_catalog_hash: state
                    .view()
                    .runtime_catalog_hash()
                    .expect("native pre-transition runtime root"),
                dataspace_additions: vec![RuntimeDataSpaceAdditionV1 {
                    descriptor: DataSpaceMetadata {
                        id: grant.dataspace.dataspace_id,
                        alias: AUTONOMOUS_RUNTIME_DATASPACE.to_owned(),
                        description: Some("autonomous native catalog effect".to_owned()),
                        fault_tolerance: 1,
                    },
                    manifest_hash: grant.name_hash,
                }],
                lane_additions: vec![LaneConfig {
                    id: AUTONOMOUS_RUNTIME_LANE,
                    alias: "catalog-merge".to_owned(),
                    dataspace_id: grant.dataspace.dataspace_id,
                    visibility: LaneVisibility::Restricted,
                    ..LaneConfig::default()
                }],
                manifest_additions: vec![RuntimeLaneManifestV1 {
                    lane_id: AUTONOMOUS_RUNTIME_LANE,
                    manifest: iroha_primitives::json::Json::new(norito::json!({
                        "lane": "catalog-merge", "version": 1,
                        "validators": validators, "quorum": 3,
                    })),
                }],
            }
            .into_custom_parameter()
            .expect("native typed catalog parameter")
        }
    };
    let mut transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(Duration::from_millis(u64::from(tag).saturating_add(1)));
    TransactionEntrypoint::External(
        transaction
            .with_instructions([iroha_data_model::isi::SetParameter::new(
                iroha_data_model::parameter::Parameter::Custom(parameter),
            )])
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            )
            .sign(keypair.private_key()),
    )
}

fn install_native_runtime_startup_registry(state: &State, keys: &[KeyPair]) {
    let nexus = state.nexus_snapshot();
    let validators = keys
        .iter()
        .map(|key| {
            let validator = AccountId::new(key.public_key().clone()).to_string();
            let peer_id = PeerId::new(key.public_key().clone()).to_string();
            norito::json!({ "validator": validator, "peer_id": peer_id })
        })
        .collect::<Vec<_>>();
    let directory = tempfile::tempdir().unwrap();
    for lane in nexus.lane_catalog.lanes() {
        let alias = lane.alias.clone();
        let lane_validators = validators.clone();
        let manifest = norito::json!({
            "lane": alias, "version": 1,
            "validators": lane_validators, "quorum": 3,
        });
        std::fs::write(
            directory
                .path()
                .join(format!("{}.manifest.json", lane.alias)),
            norito::json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
    }
    let registry = Arc::new(LaneManifestRegistry::from_config(
        &nexus.lane_catalog,
        &nexus.governance,
        &iroha_config::parameters::actual::LaneRegistry {
            manifest_directory: Some(directory.path().to_path_buf()),
            ..Default::default()
        },
    ));
    assert!(registry.is_bound_to_catalog(&nexus.lane_catalog));
    state.install_lane_manifests(&registry);
}

fn autonomous_native_runtime_effect_fixture(
    effect: AutonomousRuntimeEffectFixture,
) -> (Box<NativeEconomicFixture>, SignedBlock, HeightContext) {
    native_publication_fixture_for_test(&[NativeEconomicCase::RuntimeEffect(effect)])
}

fn assert_native_application_recorded_for_test(state: &State, carrier: &SignedBlock) {
    let batch = carrier
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_deref()
        .unwrap();
    let identity = super::lane_decision_batch::native_application_identity(
        &carrier.header(),
        batch.canonical_hash().unwrap(),
    );
    let key = StatePath::from_str(&format!(
        "native_lane_application_{}",
        hex::encode(identity.as_ref())
    ))
    .unwrap();
    assert_eq!(
        state.world.smart_contract_state.view().get(&key),
        Some(&norito::encode_canonical(&identity).unwrap()),
        "exact once-only native application marker"
    );
}

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
    let admission =
        validated_queue_plan_input_certificate_for_state_test(&state.network_id, certificate)
            .expect("fixture QueuePlan admission certificate");
    state
        .install_queue_plan_pending_binding_for_test(&admission.certificate.binding)
        .expect("fixture exact ranked QueuePlan admission and pending obligation");
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
    let admission =
        validated_queue_plan_input_certificate_for_state_test(&state.network_id, certificate)
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
        let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(network_id, 100, &roster);
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
            kagemusha_mint_finality_authorization,
            kagemusha_mint_finality_authority,
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
/// Retain actual lane execution evidence for structural/extraction component tests.
/// These tests do not publish or execute the historical MergeQC carrier: current
/// economics belongs to the Native Decision owner exercised by the roundtrips.
fn autonomous_transfer_evidence_fixture(
    mode: QueuePlanTransferFixture,
) -> (State, MergeLedgerEntry, SignedBlock) {
    let UnpersistedAutonomousMergeFixture {
        state,
        entry,
        carrier,
        ..
    } = unpersisted_autonomous_merge_commit_fixture(false, false, Some(mode), false, None, false);
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
    autonomous_merge_commit_authorization_fixture_with_runtime_effect(
        seed_expired_axt_replay,
        seed_due_start_effect,
        transfer_fixture,
        wrap_in_sealed_reveal,
        None,
    )
}

fn install_exact_merge_beacon_fixture(
    state: &State,
    world: &mut WorldBlock<'_>,
    validators: &[KeyPair],
    parent: &SignedBlock,
) -> iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1 {
    use crate::governance::parliament::{
        PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1, ParliamentAttemptStateV1,
        parliament_attempt_policy_v1,
    };
    use iroha_data_model::{
        consensus::GlobalThresholdBeaconChainAnchorV1,
        governance::types::{
            BeaconPulseId, BeaconSessionId, BodyElectionAttemptId, GovernanceAttemptId,
            GovernanceAttemptStatusV1, GovernanceAttemptV1, GovernanceExpectedHeadAbsentV1,
            GovernanceExpectedHeadV1, GovernanceStageV1, ProposalContentId, SortitionRequestV1,
            parliament_candidate_root_v1,
        },
        isi::governance::ParliamentSortitionRequestRegistrationV1,
    };
    let mut roster = validators
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    roster.sort();
    let height = parent.header().height().get() + 1;
    let (key, pulses) = crate::beacon::signed_pulses_fixture_for_roster_and_anchors(
        *state.network_id_ref(),
        &roster,
        &[
            GlobalThresholdBeaconChainAnchorV1 {
                height: parent.header().height().get() - 1,
                block_hash: parent
                    .header()
                    .prev_block_hash()
                    .expect("fixture parent has predecessor"),
            },
            GlobalThresholdBeaconChainAnchorV1 {
                height: height - 1,
                block_hash: parent.hash(),
            },
        ],
    );
    let prior = pulses[0];
    let next = pulses[1];
    let link = crate::beacon::validate_persisted_global_threshold_beacon_pulse_v1(&prior)
        .expect("real roster-bound prior pulse");
    let proposal = indexed_deploy_contract_proposal(1);
    let proposal_content_id = ProposalContentId::new(proposal.kind.fingerprint());
    let attempt_id = GovernanceAttemptId::derive_v1(proposal_content_id, 0);
    let (risk_tier, requirements) = parliament_attempt_policy_v1(&proposal.kind);
    let mut attempt = ParliamentAttemptStateV1::try_new(
        GovernanceAttemptV1 {
            id: attempt_id,
            proposal_content_id,
            sequence: 0,
            risk_tier,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        },
        PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
        height - 1,
        proposal.kind.effect_preimage_hash_v1(),
        GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
            subject_id: proposal
                .kind
                .governed_subject_id_v1()
                .expect("exact proposal subject"),
        }),
        requirements.clone(),
    )
    .expect("native pending Parliament attempt");
    attempt
        .complete_qualification(attempt_id)
        .expect("qualified Parliament attempt");
    let mut candidates = roster
        .iter()
        .map(|peer| AccountId::new(peer.public_key().clone()))
        .collect::<Vec<_>>();
    candidates.sort();
    let registrations = requirements
        .iter()
        .map(|requirement| {
            let body = requirement.body;
            let request = SortitionRequestV1::try_new_canonical(
                attempt_id,
                BodyElectionAttemptId::derive_v1(attempt_id, body, 0),
                body,
                parliament_candidate_root_v1(attempt_id, body, &candidates),
                u32::try_from(candidates.len()).expect("four native validator candidates"),
                u32::try_from(crate::governance::draw::body_committee_size(
                    &state.gov, body,
                ))
                .expect("configured Parliament body target fits u32"),
                proposal.created_height,
                height,
                BeaconSessionId::for_network_v1(state.network_id_ref()),
                None,
            )
            .expect("exact committed request for carrier-height pulse");
            ParliamentSortitionRequestRegistrationV1 {
                sequence: 0,
                request,
            }
        })
        .collect::<Vec<_>>();
    let mut request_ids = registrations
        .iter()
        .map(|registration| registration.request.id)
        .collect::<Vec<_>>();
    request_ids.sort();
    // The typed proposal determines the complete initial body pipeline. Persist
    // every request together so snapshot restore verifies that same policy.
    attempt
        .register_sortition_request_batch(attempt_id, registrations, candidates)
        .expect("complete native pending request batch");
    attempt
        .validate()
        .expect("canonical pending Parliament state");
    attempt
        .validate_proposal_bindings_v1(&proposal.kind)
        .expect("pending attempt retains the exact typed proposal policy");
    // Prove the exact candidates and configured target admit a native assignment
    // using the genuine next pulse, without consuming the persisted pending slot.
    let mut drawn = attempt.clone();
    drawn
        .consume_sortition_pulse_batch(
            attempt_id,
            request_ids,
            BeaconSessionId::for_network_v1(state.network_id_ref()),
            height,
            BeaconPulseId::new(next.pulse_id),
            crate::beacon::global_threshold_beacon_governance_seed_v1(&next, height),
            state.network_id_ref(),
            &state.gov,
        )
        .expect("real pulse admits the configured native assignment");
    drawn
        .validate()
        .expect("drawn assignment satisfies native invariants");
    drawn
        .validate_proposal_bindings_v1(&proposal.kind)
        .expect("drawn assignment retains the exact typed proposal policy");
    world
        .governance_proposals
        .insert(*proposal_content_id.as_bytes(), proposal);
    let old_session = *world
        .global_beacon_active_session
        .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
        .expect("existing queue-plan fixture session");
    world.global_beacon_key_sessions.remove(old_session);
    let old_pulses = world
        .global_beacon_pulses
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    for id in old_pulses {
        world.global_beacon_pulses.remove(id);
    }
    let old_slots = world
        .global_beacon_pulse_slots
        .iter()
        .map(|(slot, _)| *slot)
        .collect::<Vec<_>>();
    for slot in old_slots {
        world.global_beacon_pulse_slots.remove(slot);
    }
    world
        .global_beacon_key_sessions
        .insert(key.session.session_id, key);
    world
        .global_beacon_active_session
        .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, prior.session_id);
    world.global_beacon_pulses.insert(prior.pulse_id, prior);
    world.global_beacon_pulse_slots.insert(
        (
            BeaconSessionId::for_network_v1(&prior.network_id),
            prior.height,
        ),
        prior.pulse_id,
    );
    world
        .global_beacon_latest_pulse
        .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, link);
    {
        let mut transaction = world.transaction_without_telemetry(
            iroha_config::parameters::actual::LaneConfig::default(),
            0,
        );
        transaction
            .put_parliament_attempt(attempt)
            .expect("persist native required-slot indexes");
        transaction.apply();
    }
    next
}

// The native input Decision is the only economic source authority. The pulse
// session and pending request are installed before admission finality; the
// complete applying source base therefore commits them before reconstruction.
fn autonomous_native_beacon_composition_fixture() -> (
    Box<NativeEconomicFixture>,
    SignedBlock,
    iroha_data_model::block::consensus_v2::HeightContext,
) {
    let mut pulse = None;
    let fixture = native_economic_fixture_with_initializers(
        &[NativeEconomicCase::Transfer(25)],
        false,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
        None,
        |_| {},
        |state, parent| {
            let keys = (0xD3_u8..=0xD6)
                .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
                .collect::<Vec<_>>();
            let mut world = state.world.block();
            pulse = Some(install_exact_merge_beacon_fixture(
                state, &mut world, &keys, parent,
            ));
            world.commit();
        },
    );
    let mut carrier = native_consumer_stage_carrier(&fixture);
    carrier.set_npos_consensus_effects(Some(iroha_data_model::consensus::NposConsensusEffects {
        finalized_global_beacon_pulse: pulse,
        ..Default::default()
    }));
    let key = merge_carrier_finality_fixture_keypair();
    carrier
        .replace_signatures(BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(key.private_key(), carrier.hash()),
            ),
        ]))
        .unwrap();
    let parent = fixture
        .native
        .state
        .kura
        .v2_finality_artifact(fixture.native.block.header().height().get())
        .unwrap()
        .unwrap();
    let context = crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&fixture.native.state)
            .expect("derive Native applying policy from its exact committed predecessor"),
        None,
    )
    .unwrap();
    (fixture, carrier, context)
}

fn autonomous_merge_commit_authorization_fixture_with_runtime_effect(
    seed_expired_axt_replay: bool,
    seed_due_start_effect: bool,
    transfer_fixture: Option<QueuePlanTransferFixture>,
    wrap_in_sealed_reveal: bool,
    runtime_effect: Option<AutonomousRuntimeEffectFixture>,
) -> (
    State,
    MergeLedgerEntry,
    SignedBlock,
    Option<AxtHandleReplayKey>,
) {
    autonomous_merge_commit_authorization_fixture_with_beacon(
        seed_expired_axt_replay,
        seed_due_start_effect,
        transfer_fixture,
        wrap_in_sealed_reveal,
        runtime_effect,
        false,
    )
}

/// Exact certified source and carrier before output validation or durability.
/// This grants no global finality, output seal or publication authorization.
struct UnpersistedAutonomousMergeFixture {
    state: State,
    entry: MergeLedgerEntry,
    carrier: SignedBlock,
    parent: SignedBlock,
    validator_keypairs: Vec<KeyPair>,
    expired_axt_replay_key: Option<AxtHandleReplayKey>,
    requested_beacon: Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
}

/// Produce the same actual RS16 source and certified merge candidate used by
/// the full fixture, stopping before its separately owned carrier output phase.
fn unpersisted_autonomous_merge_commit_fixture(
    seed_expired_axt_replay: bool,
    seed_due_start_effect: bool,
    transfer_fixture: Option<QueuePlanTransferFixture>,
    wrap_in_sealed_reveal: bool,
    runtime_effect: Option<AutonomousRuntimeEffectFixture>,
    with_beacon: bool,
) -> UnpersistedAutonomousMergeFixture {
    let (mut state, validator_keypairs, commit_keypairs, parent) = if runtime_effect.is_some() {
        configured_runtime_effect_queue_plan_state()
    } else {
        configured_single_lane_queue_plan_state()
    };
    // Install the exact public session/history/request before any admission,
    // pre-execution or QC commits the parent state. Never mutate the certified base.
    let requested_beacon = with_beacon.then(|| {
        let mut world = state.world.block();
        let pulse =
            install_exact_merge_beacon_fixture(&state, &mut world, &validator_keypairs, &parent);
        world.commit();
        pulse
    });
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
    let entrypoint = if let Some(effect) = runtime_effect {
        autonomous_runtime_effect_entrypoint(&mut state, &validator_keypairs, tag, effect)
    } else {
        match transfer_fixture {
            Some(fixture) => queue_plan_transfer_entrypoint_for_state_test(&state, tag, fixture),
            None => queue_plan_entrypoint_for_state_test(&state, tag),
        }
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
        u64::try_from(parent.header().creation_time().as_millis())
            .expect("fixture parent time fits u64")
            .saturating_add(1),
        0,
    );
    let batch = state
        .build_merge_execution_batch_from_source_prefix(1, application_header, vec![source])
        .expect("fixture hash admission")
        .expect("fixture source produces a canonical autonomous execution batch");
    if runtime_effect.is_some() {
        for lane in &batch.lanes {
            assert!(
                lane.results.iter().all(|result| result.0.is_ok()),
                "native runtime-effect source rejected on lane {}: {:?}",
                lane.proposal.descriptor.lane_id,
                lane.results,
            );
        }
    }
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
    let carrier = certified_merge_carrier_after(&parent, &entry);
    UnpersistedAutonomousMergeFixture {
        state,
        entry,
        carrier,
        parent,
        validator_keypairs,
        expired_axt_replay_key,
        requested_beacon,
    }
}

fn autonomous_merge_commit_authorization_fixture_with_beacon(
    seed_expired_axt_replay: bool,
    seed_due_start_effect: bool,
    transfer_fixture: Option<QueuePlanTransferFixture>,
    wrap_in_sealed_reveal: bool,
    runtime_effect: Option<AutonomousRuntimeEffectFixture>,
    with_beacon: bool,
) -> (
    State,
    MergeLedgerEntry,
    SignedBlock,
    Option<AxtHandleReplayKey>,
) {
    let UnpersistedAutonomousMergeFixture {
        state,
        entry,
        mut carrier,
        parent,
        validator_keypairs,
        expired_axt_replay_key,
        requested_beacon,
    } = unpersisted_autonomous_merge_commit_fixture(
        seed_expired_axt_replay,
        seed_due_start_effect,
        transfer_fixture,
        wrap_in_sealed_reveal,
        runtime_effect,
        with_beacon,
    );
    if transfer_fixture.is_some() || runtime_effect.is_some() || wrap_in_sealed_reveal {
        // Setting the certified execution context leaves a resultless proposal.
        // Only the actual execution owner may attach its rows, fragment count,
        // transcripts and policy; a fixture cannot copy nonexistent results.
        let mut staged = state
            .block_with_certified_merge_entry(carrier.header(), &entry, ConsensusMode::Permissioned)
            .expect("stage the exact native runtime-effect carrier and source");
        let validated = ValidBlock::validate_unchecked(carrier, &mut staged).unpack(|_| {});
        carrier = validated.into();
        let committed_fragments = carrier
            .committed_fragment_count()
            .expect("actual validation attaches the fragment count");
        if transfer_fixture.is_some() || runtime_effect.is_some() {
            assert!(
                committed_fragments > 0,
                "successful source must commit a fragment"
            );
        }
        assert_eq!(
            committed_fragments,
            u64::try_from(staged.committed_fragment_count())
                .expect("native runtime-effect fragment count fits u64"),
            "the carrier retains exactly its actual executed fragments"
        );
        drop(staged);
    }
    if let Some(pulse) = requested_beacon {
        carrier.set_npos_consensus_effects(Some(
            iroha_data_model::consensus::NposConsensusEffects {
                finalized_global_beacon_pulse: Some(pulse),
                ..Default::default()
            },
        ));
        // The complete authenticated execution owner must attach outputs after
        // this proposal mutation; old result vectors cannot be retained.
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
fn stage_exact_autonomous_carrier_membership_for_pre_vote(
    state_block: &mut StateBlock<'_>,
    carrier: &SignedBlock,
) {
    let height = autonomous_carrier_transaction_height(state_block);
    state_block
        .stage_canonical_carrier_membership(carrier.network_input_hashes(), height)
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
