struct CanonicalAutonomousReplicaFixture {
    _temp_dir: TempDir,
    config: KuraConfig,
    lane_config: RuntimeLaneConfig,
    kura: Arc<Kura>,
    validators: Vec<KeyPair>,
    network_id: NetworkId,
    epoch: u64,
    payload: LaneExecutablePayloadV1,
    certified: CertifiedLaneBlockArtifact,
    alternate_certified: CertifiedLaneBlockArtifact,
    source: DurableAutonomousLaneMergeSource,
    carrier: Arc<SignedBlock>,
}

fn canonical_autonomous_replica_certificate_for_signers(
    payload: &LaneExecutablePayloadV1,
    validators: &[KeyPair],
    signer_indices: &[usize],
) -> CertifiedLaneBlockArtifact {
    let proposal = &payload.origin_proposal;
    let validator_set = validators
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    assert_eq!(validator_set, proposal.descriptor.validator_set);
    let validator_set_pops = validators
        .iter()
        .map(|keypair| {
            bls_normal_pop_prove(keypair.private_key()).expect("canonical replica validator PoP")
        })
        .collect::<Vec<_>>();
    let availability_body = crate::lane_consensus::lane_payload_availability_body(
        payload,
        proposal,
        payload.network_id,
        payload.epoch,
    )
    .expect("canonical replica availability body");
    let prepare_votes = signer_indices
        .iter()
        .map(|index| {
            let keypair = &validators[*index];
            let availability_vote =
                crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                    availability_body.clone(),
                    PeerId::new(keypair.public_key().clone()),
                    validator_set_pops.clone(),
                    keypair.private_key(),
                )
                .expect("canonical replica READY vote");
            let mut vote = signed_lane_block_vote_for_kura(proposal, CertPhase::Prepare, keypair);
            vote.payload_availability_vote = Some(availability_vote);
            vote
        })
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        validator_set.clone(),
        &prepare_votes,
    )
    .expect("canonical replica Prepare QC");
    let commit_votes = signer_indices
        .iter()
        .map(|index| {
            signed_lane_block_vote_for_kura(proposal, CertPhase::Commit, &validators[*index])
        })
        .collect::<Vec<_>>();
    let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Commit),
        validator_set,
        &commit_votes,
    )
    .expect("canonical replica Commit QC");
    let signer_pops = signer_indices
        .iter()
        .map(|index| {
            let keypair = &validators[*index];
            (
                keypair.public_key().clone(),
                bls_normal_pop_prove(keypair.private_key())
                    .expect("canonical replica selected signer PoP"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    CertifiedLaneBlockArtifact::new(
        crate::lane_consensus::CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc,
            commit_qc,
        },
        signer_pops,
    )
}

fn canonical_autonomous_replica_new_view_certificate(
    payload: &LaneExecutablePayloadV1,
    validators: &[KeyPair],
    signer_indices: &[usize],
) -> DurableLaneBlockNewViewCertificateV1 {
    let source = &payload.origin_proposal;
    let target_view = source
        .descriptor
        .lane_block_view
        .checked_add(1)
        .expect("canonical replica fixture view");
    let body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        source,
        payload,
        target_view,
        payload.network_id,
        payload.epoch,
    )
    .expect("canonical replica NewView body");
    let votes = signer_indices
        .iter()
        .map(|index| {
            let keypair = &validators[*index];
            crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
                body.clone(),
                PeerId::new(keypair.public_key().clone()),
                keypair.private_key(),
            )
            .expect("canonical replica NewView vote")
        })
        .collect::<Vec<_>>();
    let certificate = crate::lane_consensus::aggregate_lane_block_new_view_votes(
        body,
        source.descriptor.validator_set.clone(),
        &votes,
    )
    .expect("canonical replica NewView certificate");
    DurableLaneBlockNewViewCertificateV1 {
        certificate,
        signer_pops: signer_indices
            .iter()
            .map(|index| {
                let keypair = &validators[*index];
                (
                    keypair.public_key().clone(),
                    bls_normal_pop_prove(keypair.private_key())
                        .expect("canonical replica NewView signer PoP"),
                )
            })
            .collect(),
    }
}

fn canonical_autonomous_replica_fixture() -> CanonicalAutonomousReplicaFixture {
    let (temp_dir, config, lane_config) = autonomous_lane_storage_fixture();
    let lane_id = LaneId::new(1);
    let dataspace_id = lane_config.entry(lane_id).expect("lane entry").dataspace_id;
    let mut validators = (0..4)
        .map(|_| checked_keypair_with_algorithm(Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    validators.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let (_, _, seed) = autonomous_lane_payload_for_kura(lane_id, dataspace_id, 1, &validators[0]);
    let mut reproposed = repropose_autonomous_lane_payload_for_kura(&seed, 1, &validators[0]);
    reproposed.origin_proposal.payload_block_hint = None;
    let validator_set = validators
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    reproposed.origin_proposal.descriptor.validator_set = validator_set.clone();
    reproposed.origin_proposal.descriptor.validator_set_hash = HashOf::new(&validator_set);
    reproposed.origin_proposal.descriptor.validator_count = 4;
    reproposed.origin_proposal.descriptor.min_quorum = 3;
    reproposed.origin_proposal.descriptor.descriptor_hash = reproposed
        .origin_proposal
        .descriptor
        .computed_descriptor_hash();
    reproposed.origin_proposal.proposal_hash = reproposed.origin_proposal.computed_proposal_hash();
    let producer = crate::lane_consensus::deterministic_lane_author(
        &validator_set,
        reproposed.origin_proposal.descriptor.lane_block_height,
    )
    .expect("four-validator canonical replica has a deterministic producer")
    .clone();
    let producer_keypair = validators
        .iter()
        .find(|keypair| keypair.public_key() == producer.public_key())
        .expect("deterministic producer belongs to canonical replica validators");
    let network_id = test_network_id(b"kura-v2-finality-test");
    let epoch = 0;
    let context_probe = DummyBlocks::new().next();
    let height_context_id = v2_finality_artifact_for_block(&context_probe)
        .height_context
        .id();
    let (reservation_owner_hash, proposal_identity_hash) =
        autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            height_context_id,
            epoch,
            &reproposed.origin_proposal,
            &producer,
        )
        .expect("derive canonical replica height-context reservation identity");
    for reservation in &mut reproposed.reservation_keys {
        reservation.proposal_identity_hash = proposal_identity_hash;
        reservation.reservation_owner_hash = reservation_owner_hash;
    }
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        reproposed.origin_proposal,
        reproposed.entrypoints,
        reproposed.reservation_keys,
        reproposed.routing_plans,
        reproposed.native_amx_receipts,
        producer,
        producer_keypair.private_key(),
    )
    .expect("canonical-replica payload");
    let envelope =
        crate::lane_consensus::autonomous_lane_payload_envelope(&payload, network_id, epoch)
            .expect("canonical-replica envelope");
    let mut block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .with_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_autonomous_lane_payloads(vec![envelope]),
        ))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    attach_ok_results_to_block(&mut block);
    let hint = iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
        proposal_height: 1,
        proposal_view: block.header().view_change_index(),
        proposal_block_hash: block.hash(),
    };
    let payload = payload
        .attach_global_hint_exact(hint, network_id, epoch)
        .expect("attach exact canonical carrier hint");
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config).expect("Kura");
    let configured_catalog_hash = kura
        .configured_lane_catalog_baseline()
        .expect("read configured catalog baseline")
        .expect("configured catalog baseline");
    let primary = lane_config.primary();
    let primary_incarnation = Hash::new(
        format!(
            "kura-lane-incarnation:{}:{}",
            primary.lane_id.as_u32(),
            primary.dataspace_id.as_u64()
        )
        .as_bytes(),
    );
    kura.establish_or_verify_configured_primary_geometry_anchor(
        primary,
        primary_incarnation,
        configured_catalog_hash,
    )
    .expect("bind configured primary before populating its block store");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    let carrier = Arc::new(block);
    kura.store_block(Arc::clone(&carrier))
        .expect("store canonical autonomous carrier");
    persist_v2_finality_chain_through(&kura, nonzero!(1_usize));
    let certified =
        canonical_autonomous_replica_certificate_for_signers(&payload, &validators, &[0, 1, 2]);
    let alternate_certified =
        canonical_autonomous_replica_certificate_for_signers(&payload, &validators, &[1, 2, 3]);
    Kura::validate_certified_lane_block_artifact(&certified)
        .expect("canonical-replica certificate validates");
    Kura::validate_certified_lane_block_artifact(&alternate_certified)
        .expect("alternate canonical-replica certificate validates");
    assert_ne!(certified.prepare_qc, alternate_certified.prepare_qc);
    assert_ne!(certified.commit_qc, alternate_certified.commit_qc);
    let source = kura
        .persist_canonical_autonomous_lane_replica(&certified)
        .expect("persist canonical autonomous replica");
    CanonicalAutonomousReplicaFixture {
        _temp_dir: temp_dir,
        config,
        lane_config,
        kura,
        validators,
        network_id,
        epoch,
        payload,
        certified,
        alternate_certified,
        source,
        carrier,
    }
}

fn install_canonical_replica_terminal_merge_entry_for_test(
    kura: &Kura,
    replica_carrier: &SignedBlock,
    executions: Vec<MergeLaneExecution>,
    epoch: u64,
) -> MergeLedgerEntry {
    let mut raw_carrier: SignedBlock =
        BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(
                replica_carrier.header().height().get(),
                Some(replica_carrier),
            )
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
    attach_ok_results_to_block(&mut raw_carrier);
    let raw_carrier = Arc::new(raw_carrier);
    let entrypoint_count = executions
        .iter()
        .try_fold(0_u64, |count, execution| {
            count.checked_add(u64::try_from(execution.entrypoints.len()).ok()?)
        })
        .expect("canonical replica terminal entrypoint count fits u64");
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"canonical replica terminal base state"));
    let write_set_root = Hash::new(b"canonical replica terminal write set");
    let mut batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 1,
        base_state_hash,
        application_block_header: crate::merge::merge_application_header_from_carrier(
            &raw_carrier.header(),
        ),
        execution_root: crate::merge::merge_execution_root(&executions),
        entrypoint_count,
        entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(&executions)
            .expect("canonical replica terminal carrier has entrypoints"),
        result_merkle_root: crate::merge::merge_execution_result_merkle_root(&executions)
            .expect("canonical replica terminal carrier has results"),
        lanes: executions,
        application_write_set_root: Hash::new(b"canonical replica terminal application writes"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            1,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    let mut entry = sample_merge_entry(epoch);
    entry.epoch_id = epoch;
    entry.execution_batch = Some(batch);
    let carrier = bind_merge_entry_to_carrier(raw_carrier, &mut entry);
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("store canonical replica terminal merge carrier");
    persist_v2_finality_chain_through(
        kura,
        NonZeroUsize::new(
            usize::try_from(carrier.header().height().get())
                .expect("canonical replica terminal carrier height fits usize"),
        )
        .expect("canonical replica terminal carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts(
        &entry,
        carrier.header().height().get(),
        carrier.hash(),
    )
    .expect("persist canonical replica terminal application receipt");
    entry
}

fn evict_canonical_replica_terminal_carrier_to_local_sidecar(
    fixture: &mut CanonicalAutonomousReplicaFixture,
    merge_entry: &MergeLedgerEntry,
) -> (NonZeroUsize, PathBuf) {
    let carrier_height = NonZeroUsize::new(
        usize::try_from(merge_entry.merge_qc.carrier_height)
            .expect("terminal carrier height fits usize"),
    )
    .expect("terminal carrier height is non-zero");
    fixture.config.blocks_in_memory = nonzero!(1_usize);
    Arc::get_mut(&mut fixture.kura)
        .expect("terminal carrier eviction fixture keeps exclusive Kura ownership")
        .blocks_in_memory = nonzero!(1_usize);
    let carrier = fixture
        .kura
        .get_block(carrier_height)
        .expect("read terminal merge carrier before eviction");
    let mut tail: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(carrier.header().height().get(), Some(carrier.as_ref()))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    attach_ok_results_to_block(&mut tail);
    let tail = Arc::new(tail);
    fixture
        .kura
        .store_block(Arc::clone(&tail))
        .expect("store terminal carrier eviction tail");
    persist_v2_finality_chain_through(
        &fixture.kura,
        NonZeroUsize::new(
            usize::try_from(tail.header().height().get())
                .expect("terminal carrier eviction tail height fits usize"),
        )
        .expect("terminal carrier eviction tail height is non-zero"),
    );
    let wire_len = fixture
        .kura
        .advertise_required_replicas_for_bench(carrier_height)
        .expect("terminal carrier has exact selected-keeper adverts");
    assert!(
        fixture
            .kura
            .evict_block_bodies_for_bench(wire_len)
            .expect("evict terminal carrier inline body")
            >= wire_len,
    );
    let carrier_hash = fixture
        .kura
        .block_hash_at_height(carrier_height)
        .expect("resolve evicted terminal carrier hash");
    assert!(matches!(
        fixture.kura.block_body_status_by_hash(carrier_hash),
        Some(BlockBodyStatus::LocalSidecar),
    ));
    let da_path = fixture
        .kura
        .block_store
        .lock()
        .da_block_path(u64::try_from(carrier_height.get()).expect("carrier height fits u64"));
    assert!(da_path.exists());
    (carrier_height, da_path)
}

fn canonical_terminal_payload_for_replica_network_test(
    lane: &LaneConfigEntry,
    height_context_id: HeightContextId,
    network_id: NetworkId,
    epoch: u64,
    signer: &KeyPair,
) -> LaneExecutablePayloadV1 {
    let template = canonical_terminal_payload_for_test(lane, height_context_id, signer, 0xA7);
    let local_peer = PeerId::new(signer.public_key().clone());
    let (reservation_owner_hash, proposal_identity_hash) =
        autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            height_context_id,
            epoch,
            &template.origin_proposal,
            &local_peer,
        )
        .expect("derive mixed-carrier owned reservation identities");
    let mut reservations = template.reservation_keys;
    for reservation in &mut reservations {
        reservation.reservation_owner_hash = reservation_owner_hash;
        reservation.proposal_identity_hash = proposal_identity_hash;
    }
    LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        template.origin_proposal,
        template.entrypoints,
        reservations,
        template.routing_plans,
        template.native_amx_receipts,
        local_peer,
        signer.private_key(),
    )
    .expect("construct mixed-carrier owned payload on replica network")
}

fn canonical_terminal_projection_for_binding_test(
    group: LaneQueueReservationGroupBindingV1,
    binding: &AutonomousLifecycleAttemptBindingV1,
) -> ProductionInFlightFirstReleaseStateProjection {
    let mut projection = canonical_terminal_projection_for_test(group);
    let (_, _, validator_count) = binding.validator_set_identity();
    let producer = binding.producer_actor_projection();
    let validator_count =
        u8::try_from(validator_count).expect("terminal binding validator count fits u8");
    let validator_mask = if validator_count == 128 {
        u128::MAX
    } else {
        (1_u128 << validator_count) - 1
    };
    projection.validator_count = validator_count;
    projection.producer = producer;
    projection.producer_selected_owner = producer;
    projection.replicated_carrier_owners = validator_mask & !producer;
    projection.payload_binding_a = producer;
    projection.history.ever_ready_authorized = validator_mask;
    projection.history.ready_signed = validator_mask;
    projection.decision.lane_commit_owner = producer;
    projection.decision.applied_by = producer;
    assert!(production_in_flight_first_release_state_kernel(projection));
    projection
}

#[test]
fn canonical_autonomous_replica_is_idempotent_non_owning_and_restart_stable() {
    let fixture = canonical_autonomous_replica_fixture();
    let descriptor = &fixture.certified.proposal.descriptor;
    let replay = fixture
        .kura
        .persist_canonical_autonomous_lane_replica(&fixture.certified)
        .expect("exact replica replay is idempotent");
    assert_eq!(replay, fixture.source);
    assert_eq!(replay.bundle.certified, fixture.certified);
    assert_eq!(replay.bundle.executable_payload(), &fixture.payload);
    assert!(replay.bundle.autonomous.view_checkpoint.is_none());
    assert!(replay.bundle.autonomous.new_view_certificates.is_empty());
    assert_eq!(
        fixture
            .kura
            .durable_canonical_autonomous_lane_replica(
                descriptor.lane_id,
                descriptor.lane_block_height,
                fixture.network_id,
                fixture.epoch,
            )
            .expect("read exact canonical replica"),
        Some(replay.clone())
    );
    assert_eq!(
        fixture
            .kura
            .latest_canonical_autonomous_lane_replicas_matching(descriptor.lane_id, 1, |_| true,)
            .expect("read passive canonical replica suffix"),
        vec![replay.clone()]
    );
    assert!(
        fixture
            .kura
            .read_certified_lane_block_artifact_read_only(
                descriptor.lane_id,
                descriptor.lane_block_height,
            )
            .expect("read private certified slot")
            .is_none()
    );
    assert!(
        fixture
            .kura
            .read_lane_block_execution_input(descriptor.lane_id, descriptor.lane_block_height,)
            .is_none()
    );
    assert!(
        fixture
            .kura
            .read_autonomous_lane_block_artifact(
                descriptor.lane_id,
                descriptor.lane_block_height,
                fixture.network_id,
                fixture.epoch,
            )
            .is_none()
    );
    assert!(
        fixture
            .kura
            .durable_autonomous_lane_merge_source(
                descriptor.lane_id,
                descriptor.lane_block_height,
                fixture.network_id,
                fixture.epoch,
            )
            .is_err(),
        "canonical replica must not create private lifecycle custody"
    );

    let mut private_with_history = replay.bundle.clone();
    private_with_history.autonomous.new_view_certificates.push(
        canonical_autonomous_replica_new_view_certificate(
            &fixture.payload,
            &fixture.validators,
            &[0, 1, 2],
        ),
    );
    assert_ne!(
        private_with_history
            .encode_framed()
            .expect("encode private history"),
        replay.source_bundle,
        "committee-local view history must not influence canonical replica bytes"
    );
    assert_eq!(
        fixture
            .kura
            .persist_canonical_autonomous_lane_replica(&fixture.certified)
            .expect("private history does not perturb canonical replay"),
        replay
    );

    let CanonicalAutonomousReplicaFixture {
        _temp_dir,
        config,
        lane_config,
        kura,
        network_id,
        epoch,
        certified,
        source,
        ..
    } = fixture;
    let lane_id = certified.proposal.descriptor.lane_id;
    let lane_block_height = certified.proposal.descriptor.lane_block_height;
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("strict restart recovers canonical replica pair");
    reopened.replace_lane_storage_entries_for_test(&lane_config);
    assert_eq!(
        reopened
            .durable_canonical_autonomous_lane_replica(
                lane_id,
                lane_block_height,
                network_id,
                epoch,
            )
            .expect("read replica after restart"),
        Some(source)
    );
    drop(_temp_dir);
}

#[test]
fn canonical_replica_terminal_outcome_uses_nonowning_basis_without_private_custody() {
    let fixture = canonical_autonomous_replica_fixture();
    let outsider = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let outsider_peer = PeerId::new(outsider.public_key().clone());
    assert!(
        fixture
            .payload
            .origin_proposal
            .descriptor
            .validator_set
            .iter()
            .all(|validator| validator != &outsider_peer)
    );
    fixture
        .kura
        .bind_local_peer_id(outsider_peer.clone())
        .expect("bind explicit noncommittee replica peer");
    let execution = canonical_terminal_merge_execution_from_durable_source_for_test(
        &fixture.payload,
        fixture.source.clone(),
    );
    let merge_entry = install_canonical_replica_terminal_merge_entry_for_test(
        &fixture.kura,
        &fixture.carrier,
        vec![execution],
        1,
    );
    let mut publication = fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&merge_entry)
        .expect("publish noncommittee canonical replica terminal outcome")
        .expect("replica merge execution has a source outcome")
        .consume_for_v2_apply(&merge_entry)
        .expect("consume exact replica carrier publication");
    assert_eq!(publication.len(), 1);
    let (group, authorization) = publication.pop().expect("single replica authorization");
    let (authorized_group, ordered_keys, source_outcome_hash) = authorization
        .consume_for_queue()
        .expect("consume canonical replica Queue source authorization");
    assert_eq!(authorized_group, group);
    assert_eq!(ordered_keys, fixture.payload.reservation_keys);
    let descriptor = &fixture.payload.origin_proposal.descriptor;
    let entry = fixture
        .lane_config
        .entry(descriptor.lane_id)
        .expect("replica lane entry");
    let outcome_path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let pending = Kura::decode_autonomous_lifecycle_terminal_outcome(
        &outcome_path,
        &fs::read(&outcome_path).expect("read replica Pending outcome"),
    )
    .expect("decode replica Pending outcome");
    assert_eq!(pending.outcome_hash, source_outcome_hash);
    assert!(matches!(
        pending.basis(),
        AutonomousLifecycleTerminalOutcomeBasisV1::CanonicalReplica { .. }
    ));
    assert_eq!(
        pending.binding().local_validator_identity().0,
        pending.binding().producer_index,
        "replica binding uses only the producer as its logical witness",
    );
    let pending_bytes = pending
        .encode_framed()
        .expect("encode canonical basis-bearing V1 outcome");
    let decoded_pending = AutonomousLifecycleTerminalOutcomeV1::decode_framed(&pending_bytes)
        .expect("decode canonical basis-bearing V1 outcome");
    decoded_pending
        .validate_structure()
        .expect("validate canonical basis-bearing V1 outcome");
    assert_eq!(decoded_pending, pending);

    let wrong_version_body = AutonomousLifecycleTerminalOutcomeBodyV1 {
        version: AutonomousLifecycleTerminalOutcomeV1::VERSION + 1,
        binding: pending.binding().clone(),
        basis: pending.basis(),
        source: pending.source(),
        stage: pending.stage(),
    };
    let wrong_version_body_bytes = norito::encode_canonical(&wrong_version_body)
        .expect("encode wrong-version terminal outcome body");
    let wrong_version_bytes = norito::encode_canonical(&AutonomousLifecycleTerminalOutcomeV1 {
        body: wrong_version_body,
        outcome_hash: Hash::new_from_chunks(&[
            AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_HASH_DOMAIN,
            &wrong_version_body_bytes,
        ]),
    })
    .expect("encode wrong-version terminal outcome");
    assert!(
        AutonomousLifecycleTerminalOutcomeV1::decode_framed(&wrong_version_bytes).is_err(),
        "a terminal outcome outside canonical version one must fail closed",
    );

    let mut hash_substitution =
        norito::decode_canonical::<AutonomousLifecycleTerminalOutcomeV1>(&pending_bytes)
            .expect("decode canonical terminal outcome for hash substitution");
    hash_substitution.outcome_hash = Hash::new(b"substituted terminal outcome hash");
    let hash_substitution_bytes = norito::encode_canonical(&hash_substitution)
        .expect("encode terminal outcome hash substitution");
    assert!(
        AutonomousLifecycleTerminalOutcomeV1::decode_framed(&hash_substitution_bytes).is_err(),
        "an unrecomputed terminal outcome hash substitution must fail closed",
    );

    let malformed_body = AutonomousLifecycleTerminalOutcomeBodyV1 {
        version: AutonomousLifecycleTerminalOutcomeV1::VERSION,
        binding: pending.binding().clone(),
        basis: pending.basis(),
        source: AutonomousLifecycleTerminalOutcomeSourceV1::RetiredRelease {
            retirement_hash: Hash::new(b"malformed basis-bearing V1 retirement"),
        },
        stage: pending.stage(),
    };
    let malformed_body_bytes =
        norito::encode_canonical(&malformed_body).expect("encode malformed terminal outcome body");
    let malformed_bytes = norito::encode_canonical(&AutonomousLifecycleTerminalOutcomeV1 {
        body: malformed_body,
        outcome_hash: Hash::new_from_chunks(&[
            AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_HASH_DOMAIN,
            &malformed_body_bytes,
        ]),
    })
    .expect("encode malformed canonical V1 fixture");
    assert!(
        norito::decode_canonical::<AutonomousLifecycleTerminalOutcomeV1>(&malformed_bytes).is_ok(),
        "the adversarial fixture must be wire-valid canonical V1",
    );
    assert!(
        AutonomousLifecycleTerminalOutcomeV1::decode_framed(&malformed_bytes).is_err(),
        "a semantically invalid canonical V1 terminal outcome must fail closed",
    );
    #[derive(Encode)]
    enum UnknownTerminalOutcomeBasisV1 {
        #[codec(index = 2)]
        FutureReplica,
    }
    #[derive(Encode)]
    #[norito(schema_name = "iroha_core::kura::AutonomousLifecycleTerminalOutcomeBodyV1")]
    struct UnknownTerminalOutcomeBodyV1 {
        version: u16,
        binding: AutonomousLifecycleAttemptBindingV1,
        basis: UnknownTerminalOutcomeBasisV1,
        source: AutonomousLifecycleTerminalOutcomeSourceV1,
        stage: AutonomousLifecycleTerminalOutcomeStageV1,
    }
    #[derive(Encode)]
    #[norito(schema_name = "iroha_core::kura::AutonomousLifecycleTerminalOutcomeV1")]
    struct UnknownTerminalOutcomeV1 {
        body: UnknownTerminalOutcomeBodyV1,
        outcome_hash: Hash,
    }
    let unknown_basis_bytes = norito::encode_canonical(&UnknownTerminalOutcomeV1 {
        body: UnknownTerminalOutcomeBodyV1 {
            version: AutonomousLifecycleTerminalOutcomeV1::VERSION,
            binding: pending.binding().clone(),
            basis: UnknownTerminalOutcomeBasisV1::FutureReplica,
            source: pending.source(),
            stage: pending.stage(),
        },
        outcome_hash: pending.outcome_hash,
    })
    .expect("encode unknown terminal basis fixture");
    assert!(
        AutonomousLifecycleTerminalOutcomeV1::decode_framed(&unknown_basis_bytes).is_err(),
        "Norito must reject an unknown terminal-outcome basis tag",
    );
    assert!(
        fixture
            .kura
            .read_autonomous_lane_block_artifact(
                descriptor.lane_id,
                descriptor.lane_block_height,
                fixture.network_id,
                fixture.epoch,
            )
            .is_none(),
        "replica terminal publication must not synthesize a payload attempt",
    );
    let cursor_path = Kura::autonomous_lifecycle_cursor_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let bootstrap_path = Kura::autonomous_lifecycle_bootstrap_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let retirement_view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let claim_paths = fixture
        .payload
        .entrypoint_hashes
        .iter()
        .map(|entrypoint_hash| {
            Kura::autonomous_lane_entrypoint_claim_path(
                &fixture.kura.store_root,
                &fixture.network_id,
                entrypoint_hash,
            )
        })
        .collect::<Vec<_>>();
    assert!(
        !cursor_path.exists(),
        "replica publication must not synthesize a cursor"
    );
    assert!(
        !bootstrap_path.exists(),
        "replica publication must not synthesize a bootstrap"
    );
    assert!(
        !attempt_path.exists(),
        "replica publication must not synthesize an attempt"
    );
    assert!(
        !retirement_view_path.exists(),
        "replica publication must not synthesize an attempt view or retirement",
    );
    assert!(
        claim_paths.iter().all(|path| !path.exists()),
        "replica publication must not synthesize entrypoint claims",
    );
    let pending_inventory = fixture
        .kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory canonical replica Pending outcome");
    assert_eq!(pending_inventory.len(), 1);
    let expected_groups = pending_inventory[0]
        .pending_reservation_groups()
        .expect("canonical replica Pending inventory has exact reservation group")
        .to_vec();
    assert_eq!(expected_groups.len(), 1);
    let pending_stages = fixture
        .kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            fixture.network_id,
            &expected_groups,
        )
        .expect("verify canonical replica Pending stage");
    assert_eq!(pending_stages.len(), 1);
    assert_eq!(
        pending_stages[0].stage(),
        AutonomousLifecycleTerminalOutcomeDurableStage::Pending,
    );
    let terminal_projection =
        canonical_terminal_projection_for_binding_test(group, pending.binding());
    fixture
        .kura
        .complete_autonomous_lifecycle_terminal_outcome(
            group,
            terminal_projection,
            true,
            source_outcome_hash,
        )
        .expect("complete canonical replica terminal outcome");
    let complete_bytes = fs::read(&outcome_path).expect("read replica Complete outcome");
    assert_eq!(
        complete_bytes.len(),
        pending_bytes.len(),
        "canonical V1 Pending-to-Complete CAS must remain fixed-width",
    );
    let complete =
        Kura::decode_autonomous_lifecycle_terminal_outcome(&outcome_path, &complete_bytes)
            .expect("decode replica Complete outcome");
    assert!(complete.is_complete());
    assert_eq!(complete.basis(), pending.basis());
    assert!(!cursor_path.exists());
    let CanonicalAutonomousReplicaFixture {
        _temp_dir,
        config,
        lane_config,
        kura,
        network_id,
        epoch,
        payload,
        source,
        validators,
        ..
    } = fixture;
    drop(kura);
    let (committee_reopened, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("structural startup authenticates replica outcome before peer binding");
    committee_reopened.replace_lane_storage_entries_for_test(&lane_config);
    let committee_error = committee_reopened
        .bind_local_peer_id(PeerId::new(validators[0].public_key().clone()))
        .expect_err("peer binding must reject canonical-replica custody for a committee member");
    assert!(
        committee_error.to_string().contains(
            "committee validator cannot validate canonical replica basis as local custody"
        ),
        "unexpected committee-member replica rejection: {committee_error:?}",
    );
    drop(committee_reopened);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("strict restart authenticates replica Complete outcome before peer binding");
    reopened.replace_lane_storage_entries_for_test(&lane_config);
    reopened
        .bind_local_peer_id(outsider_peer)
        .expect("rebind exact noncommittee replica peer after restart");
    assert!(
        reopened
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inventory completed replica carrier after restart")
            .is_empty(),
        "a fully Complete replica carrier has no Pending recovery",
    );
    let complete_stages = reopened
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(network_id, &expected_groups)
        .expect("verify replica Complete stage after restart");
    assert_eq!(complete_stages.len(), 1);
    assert_eq!(
        complete_stages[0].stage(),
        AutonomousLifecycleTerminalOutcomeDurableStage::Complete,
    );
    let reopened_complete = Kura::decode_autonomous_lifecycle_terminal_outcome(
        &outcome_path,
        &fs::read(&outcome_path).expect("read replica Complete outcome after restart"),
    )
    .expect("decode replica Complete outcome after restart");
    assert!(reopened_complete.is_complete());
    assert!(matches!(
        reopened_complete.basis(),
        AutonomousLifecycleTerminalOutcomeBasisV1::CanonicalReplica { .. }
    ));
    assert!(!attempt_path.exists());
    assert!(!cursor_path.exists());
    assert!(!bootstrap_path.exists());
    assert!(!retirement_view_path.exists());
    assert!(claim_paths.iter().all(|path| !path.exists()));
    let descriptor = &payload.origin_proposal.descriptor;
    assert!(
        reopened
            .read_autonomous_lane_block_artifact(
                descriptor.lane_id,
                descriptor.lane_block_height,
                network_id,
                epoch,
            )
            .is_none(),
        "restart must not reinterpret replica evidence as an attempt or retirement",
    );
    assert_eq!(
        reopened
            .durable_canonical_autonomous_lane_replica(
                descriptor.lane_id,
                descriptor.lane_block_height,
                network_id,
                epoch,
            )
            .expect("read exact replica after terminal restart"),
        Some(source),
    );
    let mut retry = reopened
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&merge_entry)
        .expect("idempotently revalidate completed replica carrier after restart")
        .expect("completed replica carrier still yields exact source authorization")
        .consume_for_v2_apply(&merge_entry)
        .expect("consume exact completed replica carrier publication");
    assert_eq!(retry.len(), 1);
    let (retry_group, retry_authorization) = retry.pop().expect("one retry authorization");
    let (authorized_retry_group, retry_keys, retry_source_outcome_hash) = retry_authorization
        .consume_for_queue()
        .expect("consume completed replica Queue source authorization");
    assert_eq!(retry_group, authorized_retry_group);
    assert_eq!(retry_group, group);
    assert_eq!(retry_keys, payload.reservation_keys);
    reopened
        .complete_autonomous_lifecycle_terminal_outcome(
            retry_group,
            canonical_terminal_projection_for_binding_test(
                retry_group,
                reopened_complete.binding(),
            ),
            true,
            retry_source_outcome_hash,
        )
        .expect("idempotently retain replica Complete outcome after restart");
    assert!(
        Kura::decode_autonomous_lifecycle_terminal_outcome(
            &outcome_path,
            &fs::read(&outcome_path).expect("read idempotent replica Complete outcome"),
        )
        .expect("decode idempotent replica Complete outcome")
        .is_complete()
    );
    assert!(!attempt_path.exists());
    assert!(!cursor_path.exists());
    assert!(!bootstrap_path.exists());
    assert!(!retirement_view_path.exists());
    assert!(claim_paths.iter().all(|path| !path.exists()));
    drop(_temp_dir);
}

#[test]
fn canonical_replica_pending_survives_prebind_restart_and_rejects_committee_binding() {
    let fixture = canonical_autonomous_replica_fixture();
    let outsider = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let outsider_peer = PeerId::new(outsider.public_key().clone());
    fixture
        .kura
        .bind_local_peer_id(outsider_peer.clone())
        .expect("bind Pending replica outsider");
    let execution = canonical_terminal_merge_execution_from_durable_source_for_test(
        &fixture.payload,
        fixture.source.clone(),
    );
    let merge_entry = install_canonical_replica_terminal_merge_entry_for_test(
        &fixture.kura,
        &fixture.carrier,
        vec![execution],
        1,
    );
    let mut publication = fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&merge_entry)
        .expect("publish canonical replica Pending before crash")
        .expect("replica carrier has one Pending outcome")
        .consume_for_v2_apply(&merge_entry)
        .expect("consume exact pre-crash Pending publication");
    let (group, authorization) = publication.pop().expect("single Pending authorization");
    assert!(publication.is_empty());
    let (authorized_group, _, source_outcome_hash) = authorization
        .consume_for_queue()
        .expect("consume pre-crash Queue authorization");
    assert_eq!(authorized_group, group);
    let descriptor = fixture.payload.origin_proposal.descriptor.clone();
    let entry = fixture
        .lane_config
        .entry(descriptor.lane_id)
        .expect("Pending replica lane entry");
    let outcome_path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let attempt_path = Kura::autonomous_lane_block_attempt_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let cursor_path = Kura::autonomous_lifecycle_cursor_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let bootstrap_path = Kura::autonomous_lifecycle_bootstrap_path_for_entry(
        entry,
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let CanonicalAutonomousReplicaFixture {
        _temp_dir,
        config,
        lane_config,
        kura,
        network_id,
        validators,
        ..
    } = fixture;
    drop(kura);

    let (committee_reopened, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("strict pre-bind restart authenticates replica Pending evidence");
    committee_reopened.replace_lane_storage_entries_for_test(&lane_config);
    let committee_error = committee_reopened
        .bind_local_peer_id(PeerId::new(validators[0].public_key().clone()))
        .expect_err("committee member cannot bind over replica Pending custody");
    assert!(
        committee_error.to_string().contains(
            "committee validator cannot validate canonical replica basis as local custody"
        ),
        "unexpected Pending committee-binding rejection: {committee_error:?}",
    );
    drop(committee_reopened);

    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("second strict restart keeps replica Pending recoverable");
    reopened.replace_lane_storage_entries_for_test(&lane_config);
    reopened
        .bind_local_peer_id(outsider_peer)
        .expect("bind the noncommittee peer after Pending restart");
    let inventory = reopened
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("recover replica Pending inventory after restart");
    assert_eq!(inventory.len(), 1);
    let expected_groups = inventory[0]
        .pending_reservation_groups()
        .expect("restarted replica Pending has an exact group");
    assert_eq!(expected_groups.len(), 1);
    assert_eq!(expected_groups[0].binding(), group);
    let stages = reopened
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(network_id, &expected_groups)
        .expect("verify restarted replica Pending stage");
    assert_eq!(stages.len(), 1);
    assert_eq!(
        stages[0].stage(),
        AutonomousLifecycleTerminalOutcomeDurableStage::Pending,
    );
    let pending = Kura::decode_autonomous_lifecycle_terminal_outcome(
        &outcome_path,
        &fs::read(&outcome_path).expect("read restarted replica Pending"),
    )
    .expect("decode restarted replica Pending");
    reopened
        .complete_autonomous_lifecycle_terminal_outcome(
            group,
            canonical_terminal_projection_for_binding_test(group, pending.binding()),
            true,
            source_outcome_hash,
        )
        .expect("complete recovered replica Pending exactly once");
    assert!(
        Kura::decode_autonomous_lifecycle_terminal_outcome(
            &outcome_path,
            &fs::read(&outcome_path).expect("read completed recovered replica outcome"),
        )
        .expect("decode completed recovered replica outcome")
        .is_complete()
    );
    assert!(!attempt_path.exists());
    assert!(!cursor_path.exists());
    assert!(!bootstrap_path.exists());
    drop(_temp_dir);
}

#[test]
fn canonical_replica_pending_rejects_remote_only_terminal_carrier_without_mutation() {
    let mut fixture = canonical_autonomous_replica_fixture();
    let outsider = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    fixture
        .kura
        .bind_local_peer_id(PeerId::new(outsider.public_key().clone()))
        .expect("bind remote-only replica outsider");
    let execution = canonical_terminal_merge_execution_from_durable_source_for_test(
        &fixture.payload,
        fixture.source.clone(),
    );
    let merge_entry = install_canonical_replica_terminal_merge_entry_for_test(
        &fixture.kura,
        &fixture.carrier,
        vec![execution],
        1,
    );
    let (carrier_height, da_path) =
        evict_canonical_replica_terminal_carrier_to_local_sidecar(&mut fixture, &merge_entry);
    fixture
        .kura
        .remove_evicted_block_sidecar_for_testing(carrier_height)
        .expect("damage unpinned terminal carrier into RemoteOnly storage");
    assert!(!da_path.exists());
    let carrier_hash = fixture
        .kura
        .block_hash_at_height(carrier_height)
        .expect("resolve remote-only terminal carrier hash");
    assert!(matches!(
        fixture.kura.block_body_status_by_hash(carrier_hash),
        Some(BlockBodyStatus::RemoteOnly { .. }),
    ));
    let descriptor = &fixture.payload.origin_proposal.descriptor;
    let outcome_path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
        fixture
            .lane_config
            .entry(descriptor.lane_id)
            .expect("remote-only replica lane entry"),
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let error = match fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&merge_entry)
    {
        Ok(_) => panic!("RemoteOnly carrier must fail before replica Pending publication"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("requires its exact local carrier body"),
        "unexpected RemoteOnly publication rejection: {error:?}",
    );
    assert!(
        !outcome_path.exists(),
        "failed carrier preflight must leave terminal evidence byte-absent",
    );
}

#[test]
fn canonical_replica_pending_and_complete_pin_corrupt_carrier_on_strict_restart() {
    for complete_before_restart in [false, true] {
        let stage = if complete_before_restart {
            "Complete"
        } else {
            "Pending"
        };
        let mut fixture = canonical_autonomous_replica_fixture();
        let outsider = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        fixture
            .kura
            .bind_local_peer_id(PeerId::new(outsider.public_key().clone()))
            .expect("bind pinned-carrier replica outsider");
        let execution = canonical_terminal_merge_execution_from_durable_source_for_test(
            &fixture.payload,
            fixture.source.clone(),
        );
        let merge_entry = install_canonical_replica_terminal_merge_entry_for_test(
            &fixture.kura,
            &fixture.carrier,
            vec![execution],
            1,
        );
        let mut publication = fixture
            .kura
            .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&merge_entry)
            .expect("publish carrier-pinned replica Pending")
            .expect("carrier-pinned merge has one replica outcome")
            .consume_for_v2_apply(&merge_entry)
            .expect("consume carrier-pinned publication");
        let (group, authorization) = publication
            .pop()
            .expect("one carrier-pinned replica outcome");
        assert!(publication.is_empty());
        let (authorized_group, _, source_outcome_hash) = authorization
            .consume_for_queue()
            .expect("consume carrier-pinned Queue authorization");
        assert_eq!(authorized_group, group);
        let descriptor = &fixture.payload.origin_proposal.descriptor;
        let outcome_path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
            fixture
                .lane_config
                .entry(descriptor.lane_id)
                .expect("pinned-carrier replica lane entry"),
            &fixture.kura.store_root,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        );
        let pending = Kura::decode_autonomous_lifecycle_terminal_outcome(
            &outcome_path,
            &fs::read(&outcome_path).expect("read carrier-pinned Pending"),
        )
        .expect("decode carrier-pinned Pending");
        if complete_before_restart {
            fixture
                .kura
                .complete_autonomous_lifecycle_terminal_outcome(
                    group,
                    canonical_terminal_projection_for_binding_test(group, pending.binding()),
                    true,
                    source_outcome_hash,
                )
                .expect("complete carrier-pinned replica outcome");
        }
        let (carrier_height, da_path) =
            evict_canonical_replica_terminal_carrier_to_local_sidecar(&mut fixture, &merge_entry);
        assert_eq!(
            u64::try_from(carrier_height.get()).expect("carrier height fits u64"),
            merge_entry.merge_qc.carrier_height,
        );
        let canonical_bytes = fs::read(&da_path).expect("read exact pinned carrier DA body");
        let corrupt_bytes = vec![0xA5; canonical_bytes.len()];
        assert_ne!(corrupt_bytes, canonical_bytes);
        fs::write(&da_path, &corrupt_bytes).expect("corrupt pinned carrier in place");
        std::fs::File::open(&da_path)
            .expect("open corrupt pinned carrier")
            .sync_all()
            .expect("sync corrupt pinned carrier");

        let CanonicalAutonomousReplicaFixture {
            _temp_dir,
            config,
            lane_config,
            kura,
            ..
        } = fixture;
        drop(kura);
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
            Ok(_) => panic!("strict restart must reject corrupt {stage} carrier without cleanup"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("carrier DA body differs from signed finality"),
            "unexpected corrupt {stage} carrier rejection: {error:?}",
        );
        assert_eq!(
            fs::read(&da_path).expect("read preserved corrupt pinned carrier"),
            corrupt_bytes,
            "strict startup must not delete or rewrite a {stage}-pinned corrupt carrier",
        );
        drop(_temp_dir);
    }
}

#[test]
fn canonical_replica_terminal_only_capacity_is_exact_and_restart_stable() {
    let mut fixture = canonical_autonomous_replica_fixture();
    let outsider = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let outsider_peer = PeerId::new(outsider.public_key().clone());
    fixture
        .kura
        .bind_local_peer_id(outsider_peer.clone())
        .expect("bind capacity-test replica outsider");
    let execution = canonical_terminal_merge_execution_from_durable_source_for_test(
        &fixture.payload,
        fixture.source.clone(),
    );
    let merge_entry = install_canonical_replica_terminal_merge_entry_for_test(
        &fixture.kura,
        &fixture.carrier,
        vec![execution],
        1,
    );
    let descriptor = &fixture.payload.origin_proposal.descriptor;
    let receipt = fixture
        .kura
        .read_lane_block_application_receipt(descriptor.lane_id, descriptor.lane_block_height)
        .expect("read exact replica capacity receipt");
    let source = Kura::autonomous_lifecycle_terminal_source_from_merge_receipt(&receipt)
        .expect("derive exact replica capacity source");
    let pending_len = {
        let _prune_guard = fixture.kura.prune_lock.lock();
        fixture
            .kura
            .ensure_prune_recovery_not_required()
            .expect("capacity fixture has no prune recovery");
        let _canonical_chain_guard = fixture.kura.canonical_chain_lock.lock();
        let _geometry_guard = fixture.kura.lane_geometry_lock.lock();
        let entry = fixture
            .kura
            .lane_storage_entry(descriptor.lane_id)
            .expect("capacity fixture lane entry");
        let _sidecar_guard = fixture.kura.sidecar_lock.lock();
        let plan = fixture
            .kura
            .prepare_canonical_replica_terminal_outcome_pending_locked(
                &entry,
                &fixture.payload,
                source,
            )
            .expect("prepare exact replica Pending bytes");
        u64::try_from(
            plan.pending_bytes
                .expect("new replica terminal identity has Pending bytes")
                .len(),
        )
        .expect("replica Pending length fits u64")
    };
    let outcome_path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
        fixture
            .lane_config
            .entry(descriptor.lane_id)
            .expect("capacity fixture configured lane"),
        &fixture.kura.store_root,
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    assert!(!outcome_path.exists());
    let initial_disk_usage = fixture
        .kura
        .refresh_disk_usage_bytes()
        .expect("measure replica capacity physical bytes");
    let initial_terminal_reservations = fixture
        .kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure replica capacity terminal reservations");
    assert_eq!(
        initial_terminal_reservations, 0,
        "terminal-only replica state must not manufacture a missing owned slot",
    );
    let post_wsv_reservations = fixture
        .kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure replica capacity post-WSV envelope");
    let exact_limit = lifecycle_terminal_steady_capacity(
        &fixture.kura,
        initial_disk_usage,
        initial_terminal_reservations,
        post_wsv_reservations,
        "replica terminal-only pre-Pending",
    )
    .checked_add(pending_len)
    .and_then(|bytes| {
        bytes.checked_add(
            u64::try_from(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES)
                .expect("terminal CAS transient fits u64"),
        )
    })
    .expect("replica terminal-only exact limit fits u64");
    Arc::get_mut(&mut fixture.kura)
        .expect("replica capacity Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit - 1;
    let rejected = match fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&merge_entry)
    {
        Ok(_) => panic!("one byte below exact replica Pending capacity must fail atomically"),
        Err(error) => error,
    };
    assert!(
        rejected
            .to_string()
            .contains("reserved terminal or carrier capacity"),
        "unexpected replica capacity rejection: {rejected:?}",
    );
    assert!(
        !outcome_path.exists(),
        "failed full-set preflight must not publish a partial replica outcome",
    );

    Arc::get_mut(&mut fixture.kura)
        .expect("replica capacity Kura remains exclusive after rejection")
        .max_disk_usage_bytes = exact_limit;
    let mut publication = fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&merge_entry)
        .expect("exact replica terminal-only capacity admits Pending")
        .expect("exact replica carrier has a Pending outcome")
        .consume_for_v2_apply(&merge_entry)
        .expect("consume exact replica capacity publication");
    let (group, authorization) = publication.pop().expect("one replica capacity outcome");
    assert!(publication.is_empty());
    let (authorized_group, _, source_outcome_hash) = authorization
        .consume_for_queue()
        .expect("consume exact replica capacity Queue authorization");
    assert_eq!(authorized_group, group);
    assert_eq!(
        fixture
            .kura
            .autonomous_global_terminal_outcome_reserved_bytes()
            .expect("measure terminal-only Pending transient"),
        u64::try_from(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES)
            .expect("terminal CAS transient fits u64"),
        "one or many incomplete replica outcomes share exactly one CAS transient",
    );
    let pending = Kura::decode_autonomous_lifecycle_terminal_outcome(
        &outcome_path,
        &fs::read(&outcome_path).expect("read exact-cap replica Pending"),
    )
    .expect("decode exact-cap replica Pending");
    fixture
        .kura
        .complete_autonomous_lifecycle_terminal_outcome(
            group,
            canonical_terminal_projection_for_binding_test(group, pending.binding()),
            true,
            source_outcome_hash,
        )
        .expect("exact cap admits replica Pending-to-Complete CAS");
    assert_eq!(
        fixture
            .kura
            .autonomous_global_terminal_outcome_reserved_bytes()
            .expect("measure completed replica reservation"),
        0,
        "Complete replica state releases the shared CAS transient",
    );

    let CanonicalAutonomousReplicaFixture {
        _temp_dir,
        mut config,
        lane_config,
        kura,
        ..
    } = fixture;
    config.max_disk_usage_bytes = iroha_config::base::util::Bytes(exact_limit);
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("strict restart accepts completed terminal-only replica at exact cap");
    reopened.replace_lane_storage_entries_for_test(&lane_config);
    reopened
        .bind_local_peer_id(outsider_peer)
        .expect("peer-bound audit accepts completed terminal-only replica at exact cap");
    assert!(
        reopened
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inventory exact-cap completed replica")
            .is_empty()
    );
    drop(_temp_dir);
}

#[test]
fn canonical_carrier_keeps_owned_and_replica_terminal_bases_distinct() {
    let fixture = canonical_autonomous_replica_fixture();
    let owner = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let owner_peer = PeerId::new(owner.public_key().clone());
    assert!(
        fixture
            .payload
            .origin_proposal
            .descriptor
            .validator_set
            .iter()
            .all(|validator| validator != &owner_peer),
        "the locally owned lane signer must be outside the replica committee",
    );
    fixture
        .kura
        .bind_local_peer_id(owner_peer.clone())
        .expect("bind mixed-carrier local owner");
    let finality = fixture
        .kura
        .v2_finality_artifact(1)
        .expect("read mixed-carrier height finality")
        .expect("mixed-carrier height is finalized");
    let height_context_id = finality.height_context.id();
    let primary = fixture.lane_config.primary();
    let owned_payload = canonical_terminal_payload_for_replica_network_test(
        primary,
        height_context_id,
        fixture.network_id,
        fixture.epoch,
        &owner,
    );
    assert_eq!(
        owned_payload.origin_proposal.descriptor.validator_set,
        vec![owner_peer.clone()],
    );
    install_autonomous_lane_marker_for_kura(&fixture.kura, &fixture.lane_config, &owned_payload);
    let generation = fixture
        .kura
        .claim_autonomous_lifecycle_process_generation(fixture.network_id, &owner_peer)
        .expect("claim mixed-carrier owned lifecycle generation");
    let owned_execution =
        canonical_terminal_merge_execution_for_test(&fixture.kura, &owned_payload, &owner);
    let (_, owned_group) = install_live_lifecycle_cursor_for_terminal_test(
        &fixture.kura,
        &generation,
        &owned_payload,
        height_context_id,
        &owner,
    );
    let replica_execution = canonical_terminal_merge_execution_from_durable_source_for_test(
        &fixture.payload,
        fixture.source.clone(),
    );
    let merge_entry = install_canonical_replica_terminal_merge_entry_for_test(
        &fixture.kura,
        &fixture.carrier,
        vec![owned_execution, replica_execution],
        1,
    );
    let publication = fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&merge_entry)
        .expect("publish mixed owned/replica terminal outcomes")
        .expect("mixed carrier has exact source outcomes")
        .consume_for_v2_apply(&merge_entry)
        .expect("consume exact mixed-carrier publication");
    assert_eq!(publication.len(), 2);
    let mut observed_bases = Vec::new();
    for (group, authorization) in publication {
        let (authorized_group, _, outcome_hash) = authorization
            .consume_for_queue()
            .expect("consume mixed-carrier Queue source authorization");
        assert_eq!(group, authorized_group);
        let lane_entry = fixture
            .lane_config
            .entry(group.identity.lane_id)
            .expect("mixed-carrier lane entry");
        let outcome_path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
            lane_entry,
            &fixture.kura.store_root,
            group.identity.lane_block_height,
            group.identity.proposal_height,
        );
        let outcome = Kura::decode_autonomous_lifecycle_terminal_outcome(
            &outcome_path,
            &fs::read(&outcome_path).expect("read mixed-carrier Pending outcome"),
        )
        .expect("decode mixed-carrier Pending outcome");
        observed_bases.push((group.identity.lane_id, outcome.basis()));
        fixture
            .kura
            .complete_autonomous_lifecycle_terminal_outcome(
                group,
                canonical_terminal_projection_for_binding_test(group, outcome.binding()),
                true,
                outcome_hash,
            )
            .expect("complete mixed-carrier terminal outcome");
    }
    observed_bases.sort_by_key(|(lane_id, _)| lane_id.as_u32());
    assert_eq!(observed_bases.len(), 2);
    assert_eq!(observed_bases[0].0, primary.lane_id);
    assert_eq!(
        observed_bases[0].1,
        AutonomousLifecycleTerminalOutcomeBasisV1::OwnedLifecycle,
    );
    assert_eq!(
        observed_bases[1].0,
        fixture.payload.origin_proposal.descriptor.lane_id,
    );
    assert!(
        matches!(
            observed_bases[1].1,
            AutonomousLifecycleTerminalOutcomeBasisV1::CanonicalReplica { .. }
        ),
        "the noncommittee carrier member must retain its explicit replica basis",
    );
    assert_eq!(owned_group.identity.lane_id, primary.lane_id);
    assert!(
        fixture
            .kura
            .read_autonomous_lane_block_artifact(
                primary.lane_id,
                owned_payload.origin_proposal.descriptor.lane_block_height,
                fixture.network_id,
                fixture.epoch,
            )
            .is_some(),
        "the owned member retains its exact attempt custody",
    );
    assert!(
        fixture
            .kura
            .read_autonomous_lane_block_artifact(
                fixture.payload.origin_proposal.descriptor.lane_id,
                fixture.payload.origin_proposal.descriptor.lane_block_height,
                fixture.network_id,
                fixture.epoch,
            )
            .is_none(),
        "the replica member never acquires private attempt custody",
    );
}

#[test]
fn canonical_autonomous_replica_retains_first_valid_quorum_proof_variant() {
    let fixture = canonical_autonomous_replica_fixture();
    let descriptor = &fixture.certified.proposal.descriptor;
    let entry = fixture
        .lane_config
        .entry(descriptor.lane_id)
        .expect("lane entry");
    let (data_path, index_path) =
        Kura::canonical_autonomous_lane_replica_paths_for_entry(entry, &fixture.kura.store_root);
    let data_before = fs::read(&data_path).expect("read retained canonical replica data");
    let index_before = fs::read(&index_path).expect("read retained canonical replica index");

    let replay = fixture
        .kura
        .persist_canonical_autonomous_lane_replica(&fixture.alternate_certified)
        .expect("alternate valid 3-of-4 proof is an idempotent replay");
    assert_eq!(replay, fixture.source);
    assert_eq!(replay.bundle.certified, fixture.certified);
    assert_ne!(
        replay.bundle.certified, fixture.alternate_certified,
        "the first valid proof bytes remain the durable canonical source"
    );
    assert_eq!(
        fs::read(&data_path).expect("read canonical replica data after alternate replay"),
        data_before,
        "an equivalent proof must not append or replace durable replica bytes"
    );
    assert_eq!(
        fs::read(&index_path).expect("read canonical replica index after alternate replay"),
        index_before,
        "an equivalent proof must not mutate the slot index"
    );

    let retained = norito::decode_canonical::<CanonicalAutonomousLaneReplicaV1>(&data_before)
        .expect("decode retained canonical replica record");
    let mut conflicting_decision = retained.clone();
    conflicting_decision
        .bundle
        .certified
        .commit_qc
        .body
        .proposal_hash = Hash::new(b"conflicting canonical replica decision");
    assert!(
        !Kura::canonical_autonomous_lane_replicas_certify_same_decision(
            &retained,
            &conflicting_decision,
        ),
        "a different CommitQC decision body must never use proof-variant idempotence"
    );

    let CanonicalAutonomousReplicaFixture {
        _temp_dir,
        config,
        lane_config,
        kura,
        alternate_certified,
        source,
        ..
    } = fixture;
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("strict restart recovers first canonical replica proof");
    reopened.replace_lane_storage_entries_for_test(&lane_config);
    assert_eq!(
        reopened
            .persist_canonical_autonomous_lane_replica(&alternate_certified)
            .expect("alternate proof remains idempotent after restart"),
        source
    );
    drop(_temp_dir);
}

#[test]
fn canonical_autonomous_replica_corruption_and_wrong_context_fail_closed() {
    let fixture = canonical_autonomous_replica_fixture();
    let descriptor = &fixture.certified.proposal.descriptor;
    assert!(
        fixture
            .kura
            .durable_canonical_autonomous_lane_replica(
                descriptor.lane_id,
                descriptor.lane_block_height,
                test_network_id(b"wrong canonical replica network"),
                fixture.epoch,
            )
            .is_err()
    );
    assert!(
        fixture
            .kura
            .durable_canonical_autonomous_lane_replica(
                descriptor.lane_id,
                descriptor.lane_block_height,
                fixture.network_id,
                fixture.epoch.saturating_add(1),
            )
            .is_err()
    );
    let entry = fixture
        .lane_config
        .entry(descriptor.lane_id)
        .expect("lane entry");
    let (data_path, _) =
        Kura::canonical_autonomous_lane_replica_paths_for_entry(entry, &fixture.kura.store_root);
    let mut bytes = fs::read(&data_path).expect("read canonical replica data");
    let byte = bytes.last_mut().expect("non-empty canonical replica data");
    *byte ^= 0x80;
    fs::write(&data_path, bytes).expect("corrupt canonical replica data");
    assert!(
        fixture
            .kura
            .durable_canonical_autonomous_lane_replica(
                descriptor.lane_id,
                descriptor.lane_block_height,
                fixture.network_id,
                fixture.epoch,
            )
            .is_err(),
        "corrupt replica bytes must never be interpreted as absence"
    );
    assert!(
        fixture
            .kura
            .latest_canonical_autonomous_lane_replicas_matching(descriptor.lane_id, 1, |_| true,)
            .is_err(),
        "passive diagnostics must report corruption instead of skipping it"
    );
}
