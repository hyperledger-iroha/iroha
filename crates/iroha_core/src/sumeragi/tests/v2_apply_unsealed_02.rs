fn direct_archive_tempdir() -> tempfile::TempDir {
    let root = std::env::current_dir().expect("resolve direct archive test root");
    tempfile::tempdir_in(root).expect("create direct archive test directory")
}

fn provider_ingest_archive_bounds(
    max_record_bytes: u64,
    max_total_bytes: u64,
) -> ProviderIngestFinalizedArchiveBoundsV1 {
    ProviderIngestFinalizedArchiveBoundsV1::try_new(
        max_record_bytes,
        16,
        max_total_bytes,
        16,
        16,
        64,
        16,
    )
    .expect("valid provider-ingest archive bounds")
}

fn fixture_reserve_asset_definition() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("sorafs", "universal").expect("valid fixture settlement domain"),
        "xor".parse().expect("valid fixture settlement asset name"),
    )
}

fn fixture_queue(state: &State, events_sender: crate::EventsSender) -> Arc<Queue> {
    let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
    let manifests = state.lane_manifests.read().clone();
    queue.install_lane_manifests(&manifests);
    queue
}

type ApplyNativeReceiptBuilder =
    fn(
        &ApplyFixture,
        &wire::HeightContext,
        iroha_data_model::NetworkId,
        &LaneBlockProposalV1,
        &[TransactionEntrypoint],
        &[crate::queue::LaneQueueReservationKeyV2],
        &[crate::queue::RoutingPlan],
    ) -> Vec<Option<iroha_data_model::block::consensus::NativeAmxReceipt>>;

fn install_fixture_native_lane(state: &mut State, context: &mut wire::HeightContext) {
    use iroha_data_model::nexus::{
        DataSpaceCatalog, DataSpaceMetadata, LaneConfig, LaneLifecyclePlan,
    };

    let participant_lane = LaneId::new(1);
    let participant_dataspace = DataSpaceId::new(7);
    let mut nexus = state.nexus_snapshot();
    assert!(nexus.enabled, "Native fixture requires enabled Nexus");
    nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: participant_dataspace,
            alias: "independent-dataspace".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("valid Native fixture dataspace catalog");
    state
        .set_nexus(nexus)
        .expect("install Native fixture dataspace before genesis");

    let lane = LaneConfig {
        id: participant_lane,
        dataspace_id: participant_dataspace,
        alias: "independent-lane".to_owned(),
        ..LaneConfig::default()
    };
    state
        .apply_lane_lifecycle(&LaneLifecyclePlan {
            additions: vec![lane.clone()],
            retire: Vec::new(),
        })
        .expect("install Native fixture lane before genesis");

    let validators = context
        .roster
        .iter()
        .map(|validator| AccountId::new(validator.validator.public_key().clone()))
        .collect::<Vec<_>>();
    let validator_bindings = validators
        .iter()
        .zip(&context.roster)
        .map(|(validator, power)| ManifestValidatorBinding {
            validator: validator.clone(),
            peer_id: power.validator.clone(),
            torii_url: None,
        })
        .collect();
    let status = LaneManifestStatus {
        lane: lane.id,
        alias: lane.alias,
        dataspace: lane.dataspace_id,
        visibility: lane.visibility,
        storage: lane.storage,
        governance: lane.governance,
        manifest_path: Some(std::path::PathBuf::from(
            "/tmp/sumeragi-v2-apply-native-lane-manifest.json",
        )),
        governance_rules: Some(GovernanceRules {
            validators,
            validator_bindings,
            ..GovernanceRules::default()
        }),
        privacy_commitments: Vec::new(),
    };
    let mut statuses = {
        let manifests = state.lane_manifests.read();
        manifests
            .statuses()
            .into_iter()
            .map(|status| (status.lane, status))
            .collect::<BTreeMap<_, _>>()
    };
    statuses.insert(participant_lane, status);
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));

    let mut expected = context
        .roster
        .iter()
        .map(|validator| validator.validator.clone())
        .collect::<Vec<_>>();
    expected.sort();
    expected.dedup();
    let mut actual = state.authoritative_lane_peer_ids_at_height(participant_lane, context.height);
    actual.sort();
    actual.dedup();
    assert_eq!(actual, expected, "Native fixture participant authority");
    let incarnation = state
        .lane_incarnation_at_height(participant_lane, context.height)
        .expect("Native fixture participant incarnation is active");
    assert!(
        incarnation.as_ref().iter().any(|byte| *byte != 0),
        "Native fixture participant incarnation is non-zero"
    );
    context.nexus_amx_context_hash =
        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(state);
}

fn native_amx_receipts_for_apply_fixture(
    fixture: &ApplyFixture,
    context: &wire::HeightContext,
    network_id: iroha_data_model::NetworkId,
    coordinator_proposal: &LaneBlockProposalV1,
    entrypoints: &[TransactionEntrypoint],
    reservation_keys: &[crate::queue::LaneQueueReservationKeyV2],
    routing_plans: &[crate::queue::RoutingPlan],
) -> Vec<Option<iroha_data_model::block::consensus::NativeAmxReceipt>> {
    use crate::{
        native_amx::{
            NativeAmxAttestationRequestV2, NativeAmxVoteV2, aggregate_votes_to_qc,
            receipt_shape_matches_coordinator_payload,
        },
        queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan},
    };
    use iroha_data_model::block::consensus::{
        NativeAmxAttestationBodyV2, NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
        SumeragiLanePayloadOwnership,
    };

    assert_eq!(entrypoints.len(), 2, "grouped Native fixture source count");
    assert_eq!(reservation_keys.len(), entrypoints.len());
    assert_eq!(routing_plans.len(), entrypoints.len());
    let participant_lane = LaneId::new(1);
    let participant_dataspace = DataSpaceId::new(7);
    let expected_plan = RoutingPlan::native_amx(
        RoutingDecision::default(),
        vec![RouteLeg::new(
            RoutingDecision::new(participant_lane, participant_dataspace),
            RouteLegRole::Participant,
        )],
    );
    assert!(
        routing_plans.iter().all(|plan| plan == &expected_plan),
        "grouped fixture transactions derive one exact Native plan"
    );

    let source_ids = reservation_keys
        .iter()
        .map(|key| {
            let mut source_id = [0_u8; Hash::LENGTH];
            source_id.copy_from_slice(key.signed_transaction_hash.as_ref());
            source_id
        })
        .collect::<Vec<_>>();
    assert!(
        source_ids.windows(2).all(|pair| pair[0] < pair[1]),
        "grouped Native sources are in canonical signed-transaction order"
    );
    let entrypoint_hashes = entrypoints
        .iter()
        .map(TransactionEntrypoint::hash)
        .collect::<Vec<_>>();
    assert!(
        reservation_keys
            .iter()
            .zip(&entrypoint_hashes)
            .all(|(key, entrypoint_hash)| key.entrypoint_hash == *entrypoint_hash),
        "grouped Native reservations bind the typed entrypoints"
    );

    let mut validator_set = fixture
        .state
        .authoritative_lane_peer_ids_at_height(participant_lane, context.height);
    validator_set.sort();
    validator_set.dedup();
    let mut expected_validators = context
        .roster
        .iter()
        .map(|validator| validator.validator.clone())
        .collect::<Vec<_>>();
    expected_validators.sort();
    expected_validators.dedup();
    assert_eq!(
        validator_set, expected_validators,
        "participant proposal uses the frozen authoritative committee"
    );
    let validator_pops = validator_set
        .iter()
        .map(|validator| {
            let index = context
                .roster
                .iter()
                .position(|candidate| &candidate.validator == validator)
                .expect("participant validator occurs in the frozen roster");
            fixture.service.validator_set_pops[index].clone()
        })
        .collect::<Vec<_>>();
    let validator_keys = validator_set
        .iter()
        .map(|validator| {
            fixture
                .validator_keys
                .iter()
                .find(|key| key.public_key() == validator.public_key())
                .expect("participant validator has a fixture signing key")
                .clone()
        })
        .collect::<Vec<_>>();
    let min_signers =
        crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1);
    let validator_count =
        u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
    let participant_min_quorum =
        u32::try_from(min_signers).expect("fixture participant quorum fits u32");
    let participant_incarnation = fixture
        .state
        .lane_incarnation_at_height(participant_lane, context.height)
        .expect("participant incarnation is active in the frozen context");
    let base_mode_tag = match context.mode {
        wire::ConsensusMode::Permissioned => wire::PERMISSIONED_TAG,
        wire::ConsensusMode::Npos => wire::NPOS_TAG,
    };
    let context_mode_tag = format!(
        "{base_mode_tag}::height-context:{}::epoch:{}",
        hex::encode(context.id().0.as_ref()),
        context.epoch
    );
    let accepted_transaction_hashes = entrypoint_hashes
        .iter()
        .copied()
        .map(Hash::from)
        .collect::<Vec<_>>();
    let mut participant_ownership = SumeragiLanePayloadOwnership {
        proposal_height: context.height,
        proposal_view: 0,
        lane_id: participant_lane,
        dataspace_id: participant_dataspace,
        lane_incarnation: participant_incarnation,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: Hash::prehashed([0; Hash::LENGTH]),
        qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
            participant_lane,
            participant_dataspace,
            &context_mode_tag,
        ),
        accepted_candidate_indices: (0_u64..2).collect(),
        accepted_transaction_hashes,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_descriptor_hash: Some(Hash::prehashed([0; Hash::LENGTH])),
        lane_block_descriptor_validator_set: validator_set.clone(),
        lane_block_descriptor_validator_count: validator_count,
        lane_block_descriptor_min_quorum: participant_min_quorum,
        payload_ownership_hash: Hash::prehashed([0; Hash::LENGTH]),
        rbc_instance_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    let replay = participant_ownership
        .compute_replay_hashes()
        .expect("participant ownership replay material is canonical");
    participant_ownership.subject_hash = replay.subject_hash;
    participant_ownership.payload_ownership_hash = replay.payload_ownership_hash;
    participant_ownership.rbc_instance_hash = replay.rbc_instance_hash;
    participant_ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
    let mut participant_descriptor = LaneBlockDescriptorV1 {
        lane_id: participant_ownership.lane_id,
        dataspace_id: participant_ownership.dataspace_id,
        lane_incarnation: participant_ownership.lane_incarnation,
        proposal_height: participant_ownership.proposal_height,
        previous_lane_block_height: participant_ownership.previous_lane_block_height,
        previous_lane_block_descriptor_hash: participant_ownership
            .previous_lane_block_descriptor_hash,
        lane_block_height: participant_ownership.lane_block_height,
        lane_block_view: participant_ownership.lane_block_view,
        subject_hash: participant_ownership.subject_hash,
        payload_ownership_hash: participant_ownership.payload_ownership_hash,
        rbc_instance_hash: participant_ownership.rbc_instance_hash,
        accepted_candidate_indices: participant_ownership.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: participant_ownership.accepted_transaction_hashes.clone(),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count,
        min_quorum: participant_min_quorum,
        qc_mode_tag: participant_ownership.qc_mode_tag,
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    participant_descriptor.descriptor_hash = participant_descriptor.computed_descriptor_hash();
    assert_eq!(
        participant_descriptor.descriptor_hash,
        replay.lane_block_descriptor_hash
    );
    let mut participant_proposal = LaneBlockProposalV1 {
        descriptor: participant_descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    participant_proposal.proposal_hash = participant_proposal.computed_proposal_hash();
    crate::lane_consensus::validate_lane_block_proposal(&participant_proposal)
        .expect("grouped Native participant proposal is production-valid");

    let coordinator_descriptor = &coordinator_proposal.descriptor;
    assert_eq!(
        RoutingDecision::new(
            coordinator_descriptor.lane_id,
            coordinator_descriptor.dataspace_id,
        ),
        RoutingDecision::default(),
        "grouped Native coordinator remains the universal route"
    );
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let participant_validator_set_hash = HashOf::new(&validator_set);
    let body_for = |source_id, tx_entrypoint_hash, phase, participant_settlement_commitment| {
        NativeAmxAttestationBodyV2 {
            round,
            epoch: context.epoch,
            network_id,
            source_id,
            tx_entrypoint_hash,
            plan_digest: expected_plan.digest(),
            phase,
            coordinator_lane_id: coordinator_descriptor.lane_id,
            coordinator_dataspace_id: coordinator_descriptor.dataspace_id,
            coordinator_lane_incarnation: coordinator_descriptor.lane_incarnation,
            participant_lane_id: participant_lane,
            participant_dataspace_id: participant_dataspace,
            participant_lane_incarnation: participant_incarnation,
            participant_previous_block_height: 0,
            participant_previous_block_descriptor_hash: None,
            participant_lane_block_height: participant_proposal.descriptor.lane_block_height,
            participant_lane_block_view: participant_proposal.descriptor.lane_block_view,
            participant_proposal_hash: participant_proposal.proposal_hash,
            participant_settlement_commitment,
            participant_validator_set_hash,
            participant_validator_count: validator_count,
            participant_min_quorum,
            authority_context_height: context.height,
            planned_coordinator_block_height: coordinator_descriptor.lane_block_height,
            coordinator_lane_block_view: coordinator_descriptor.lane_block_view,
            coordinator_proposal_hash: coordinator_proposal.proposal_hash,
        }
    };
    let settlement_template = body_for(
        source_ids[0],
        entrypoint_hashes[0],
        NativeAmxPhase::Prepare,
        Hash::prehashed([0; Hash::LENGTH]),
    );
    let participant_settlement = settlement_template
        .computed_grouped_participant_settlement(&source_ids)
        .expect("derive exact two-source participant settlement");
    let participant_settlement_hash =
        iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
            .expect("hash exact two-source participant settlement");

    let qc_for = |body: NativeAmxAttestationBodyV2| {
        let votes = validator_keys
            .iter()
            .take(min_signers)
            .map(|key| NativeAmxVoteV2 {
                body,
                signer: PeerId::new(key.public_key().clone()),
                bls_signature: Signature::try_new(key.private_key(), &body.signature_preimage())
                    .expect("sign grouped Native participant attestation")
                    .payload()
                    .to_vec(),
            })
            .collect::<Vec<_>>();
        aggregate_votes_to_qc(
            body,
            validator_set.clone(),
            validator_pops.clone(),
            &votes,
            min_signers,
        )
        .expect("aggregate grouped Native participant QC")
    };

    source_ids
        .iter()
        .copied()
        .zip(entrypoint_hashes.iter().copied())
        .zip(routing_plans)
        .map(|((source_id, entrypoint_hash), routing_plan)| {
            let prepare_body = body_for(
                source_id,
                entrypoint_hash,
                NativeAmxPhase::Prepare,
                Hash::from(participant_settlement_hash),
            );
            let request = NativeAmxAttestationRequestV2 {
                body: prepare_body,
                plan_legs: routing_plan.legs(),
                coordinator_proposal: coordinator_proposal.clone(),
                participant_proposal: participant_proposal.clone(),
                participant_settlement: participant_settlement.clone(),
            };
            request
                .validate_plan_binding()
                .expect("grouped Native request binds the complete production plan");
            let prepare_qc = qc_for(prepare_body);
            let mut commit_body = prepare_body;
            commit_body.phase = NativeAmxPhase::Commit;
            let leg = NativeAmxLegRecordV2 {
                lane_id: participant_lane,
                dataspace_id: participant_dataspace,
                participant_proposal: participant_proposal.clone(),
                participant_settlement: participant_settlement.clone(),
                participant_settlement_hash,
                prepare_qc,
                commit_qc: qc_for(commit_body),
            };
            let receipt = NativeAmxReceipt {
                version: 2,
                source_id,
                network_id,
                plan_digest: routing_plan.digest(),
                lane_id: coordinator_descriptor.lane_id,
                dataspace_id: coordinator_descriptor.dataspace_id,
                lane_incarnation: coordinator_descriptor.lane_incarnation,
                authority_context_height: coordinator_descriptor.proposal_height,
                lane_block_height: coordinator_descriptor.lane_block_height,
                lane_block_view: coordinator_descriptor.lane_block_view,
                coordinator_proposal_hash: coordinator_proposal.proposal_hash,
                legs: vec![leg],
            };
            assert!(
                receipt_shape_matches_coordinator_payload(
                    Some(&receipt),
                    routing_plan,
                    source_id.as_slice(),
                    Hash::from(entrypoint_hash),
                    network_id,
                    coordinator_proposal,
                ),
                "grouped Native receipt matches the exact coordinator payload"
            );
            Some(receipt)
        })
        .collect()
}

v2_apply_test!(
    checkpoint_write_failure_keeps_wsv_behind_durable_kura_tip,
    {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_wsv_checkpoint_write_for_tests();
        let error = fixture
            .execute(&mut store)
            .expect_err("checkpoint failure follows the durable Kura boundary");
        assert!(
            matches!(
                &error,
                V2ApplyError::CommittedRecoveryRequired { stage, .. }
                    if *stage == "pre-WSV recovery checkpoint"
            ),
            "unexpected committed recovery classification: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(
            fixture.state.committed_height(),
            0,
            "live WSV must not advance without its durable recovery checkpoint"
        );
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            fixture
                .kura
                .commit_manifest(fixture.context.height)
                .expect("read absent manifest")
                .is_none()
        );
        assert_eq!(
            fixture
                .kura
                .v2_finality_artifact(fixture.context.height)
                .expect("read pre-WSV finality")
                .expect("finality must precede the WSV checkpoint")
                .block_hash,
            fixture.body.hash()
        );

        drop(store);
        let mut reopened = fixture.reopen_body_store();
        assert!(
            reopened
                .validated_recovery_catalog()
                .contains_key(&(fixture.manifest.round, fixture.manifest.subject)),
            "restart must recover the exact durable validation marker"
        );
        fixture
            .execute(&mut reopened)
            .expect("replay the exact durable tip and publish WSV once");
        fixture.assert_complete();
    }
);

v2_apply_test!(
    provider_ingest_archive_failure_after_kura_and_checkpoint_keeps_state_unpublished,
    {
        let mut fixture = ApplyFixture::new();
        let archive_root = direct_archive_tempdir();
        let archive = Arc::new(
            ProviderIngestFinalizedArchiveV1::try_open(
                archive_root.path(),
                provider_ingest_archive_bounds(32, 32),
            )
            .expect("open deliberately tiny provider-ingest archive"),
        );
        fixture.service.provider_ingest_finalized_archive = Some(Arc::clone(&archive));
        let mut store = fixture.reopen_body_store();

        let error = fixture
            .execute(&mut store)
            .expect_err("provider-ingest capture must exceed the tiny record bound");
        assert!(
            matches!(
                &error,
                V2ApplyError::CommittedRecoveryRequired { stage, .. }
                    if *stage == "provider-ingest finalized archive capture"
            ),
            "unexpected committed recovery classification: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read staged checkpoint")
                .is_some(),
            "WSV checkpoint must precede provider-ingest archive capture"
        );
        assert!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read durable finality")
                .is_some(),
            "authenticated Kura finality must precede provider-ingest capture"
        );
        assert_eq!(
            fixture.state.committed_height(),
            0,
            "provider-ingest archive failure must precede live State publication"
        );
        assert!(
            archive
                .activation_floor(&fixture.service.network_id)
                .expect("read empty activation floor")
                .is_none()
        );
    }
);

v2_apply_test!(
    provider_ingest_archive_recovery_replays_exact_capture_without_duplicate_generation,
    {
        let mut fixture = ApplyFixture::new();
        let archive_root = direct_archive_tempdir();
        let archive = Arc::new(
            ProviderIngestFinalizedArchiveV1::try_open(
                archive_root.path(),
                provider_ingest_archive_bounds(4 * 1024 * 1024, 64 * 1024 * 1024),
            )
            .expect("open provider-ingest archive"),
        );
        fixture.service.provider_ingest_finalized_archive = Some(Arc::clone(&archive));
        fixture
            .service
            .fail_after_provider_ingest_archive_capture_for_test();
        let mut first_store = fixture.reopen_body_store();

        let error = fixture
            .execute(&mut first_store)
            .expect_err("inject crash after provider-ingest archive capture");
        assert!(
            matches!(
                &error,
                V2ApplyError::InjectedCrashAfterProviderIngestArchiveCapture
            ),
            "unexpected provider-ingest capture result: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 0);
        let generation_after_capture = archive
            .health_generation()
            .expect("read first provider-ingest archive generation");
        let qualification = archive
            .qualify_against_kura_tip(&fixture.service.network_id, fixture.kura.as_ref(), 0)
            .expect("provider-ingest archive is exact at the durable Kura tip");
        assert_eq!(qualification.activation_floor().height, 1);
        assert_eq!(qualification.archive_tip().height, 1);
        assert_eq!(qualification.kura_tip_height(), 1);
        assert_eq!(qualification.lag_blocks(), 0);
        drop(first_store);

        let (restarted_service, restarted_state) =
            fixture.restart_service_from_last_finalized_snapshot();
        let mut restarted_store = fixture.reopen_body_store();
        restarted_service
            .execute(&fixture.context, &mut restarted_store, &fixture.task)
            .expect("restart replays the exact provider-ingest capture and publishes WSV");
        assert_eq!(restarted_state.committed_height(), 1);
        assert_eq!(
            archive
                .health_generation()
                .expect("read replayed provider-ingest archive generation"),
            generation_after_capture,
            "an exact replay must not publish another archive generation"
        );

        let (_, receipt) = restarted_service
            .kura
            .v2_finality_artifact_with_receipt(1)
            .expect("read exact finality receipt")
            .expect("height-one finality receipt");
        let restarted_view = restarted_state.query_view();
        let outcome = archive
            .capture_kura_authenticated_view(&restarted_view, fixture.kura.as_ref(), &receipt)
            .expect("same exact provider-ingest committed view is replay-safe");
        assert_eq!(
            outcome,
            ProviderIngestFinalizedArchiveInsertOutcomeV1::ExactReplay
        );
    }
);

v2_apply_test!(
    reputation_archive_failure_after_kura_and_checkpoint_keeps_state_unpublished,
    {
        let mut fixture = ApplyFixture::new_with_reputation_archive();
        let archive_root = direct_archive_tempdir();
        let bounds = ReputationFinalizedArchiveBounds::try_new(32, 16, 32)
            .expect("valid deliberately tiny archive bounds");
        let archive = Arc::new(
            ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                .expect("open tiny archive"),
        );
        fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
        let mut store = fixture.reopen_body_store();

        let error = fixture
            .execute(&mut store)
            .expect_err("archive capture must exceed the deliberately tiny bound");
        assert!(
            matches!(
                &error,
                V2ApplyError::CommittedRecoveryRequired { stage, .. }
                    if *stage == "reputation finalized archive capture"
            ),
            "unexpected committed recovery classification: {error:?}"
        );
        assert!(error.requires_restart_recovery());
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .expect("read staged checkpoint")
                .is_some(),
            "WSV checkpoint must precede archive capture"
        );
        assert!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read durable finality")
                .is_some(),
            "authenticated Kura finality must precede archive capture"
        );
        assert_eq!(
            fixture.state.committed_height(),
            0,
            "archive failure must precede live State publication"
        );
        assert!(
            archive
                .activation_floor(&fixture.service.network_id)
                .expect("read empty activation floor")
                .is_none()
        );
    }
);

v2_apply_test!(
    crash_after_reputation_archive_capture_precedes_state_block_commit,
    {
        let mut fixture = ApplyFixture::new_with_reputation_archive();
        let archive_root = direct_archive_tempdir();
        let bounds =
            ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                .expect("valid archive bounds");
        let archive = Arc::new(
            ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                .expect("open archive"),
        );
        fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
        fixture
            .service
            .fail_after_reputation_archive_capture_for_test();
        let mut store = fixture.reopen_body_store();

        let error = fixture
            .execute(&mut store)
            .expect_err("injected post-capture crash");
        assert!(
            matches!(
                &error,
                V2ApplyError::InjectedCrashAfterReputationArchiveCapture
            ),
            "unexpected post-capture result: {error:?}"
        );
        assert_eq!(
            fixture.state.committed_height(),
            0,
            "injected archive-boundary crash must precede StateBlock::commit"
        );
        let qualification = archive
            .qualify_against_kura_tip(&fixture.service.network_id, fixture.kura.as_ref(), 0)
            .expect("captured archive is exact at the durable Kura tip");
        assert_eq!(qualification.activation_floor().height, 1);
        assert_eq!(qualification.archive_tip().height, 1);
        assert_eq!(qualification.kura_tip_height(), 1);
        assert_eq!(qualification.lag_blocks(), 0);
        assert!(
            fixture
                .kura
                .commit_manifest(1)
                .expect("read absent post-apply manifest")
                .is_none(),
            "post-apply metadata must remain behind State publication"
        );
    }
);

v2_apply_test!(
    reputation_archive_recovery_is_idempotent_without_skipping_height,
    {
        let mut fixture = ApplyFixture::new_with_reputation_archive();
        let archive_root = direct_archive_tempdir();
        let bounds =
            ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                .expect("valid archive bounds");
        let archive = Arc::new(
            ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                .expect("open archive"),
        );
        fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));
        fixture
            .service
            .fail_after_reputation_archive_capture_for_test();
        let mut first_store = fixture.reopen_body_store();
        let error = fixture
            .execute(&mut first_store)
            .expect_err("injected post-capture crash");
        assert!(
            matches!(
                &error,
                V2ApplyError::InjectedCrashAfterReputationArchiveCapture
            ),
            "unexpected post-capture result: {error:?}"
        );
        let generation_after_capture = archive
            .health_generation()
            .expect("read first archive generation");
        let staged_checkpoint = fixture
            .kura
            .wsv_checkpoint(1)
            .expect("read pre-retry staged checkpoint")
            .expect("archive-boundary crash retains its staged checkpoint");
        assert!(
            fixture
                .kura
                .commit_manifest(1)
                .expect("read pre-retry commit manifest")
                .is_none(),
            "archive-boundary crash must precede commit-manifest publication"
        );
        drop(first_store);

        let (restarted_service, restarted_state) =
            fixture.restart_service_from_last_finalized_snapshot();
        let mut restarted_store = fixture.reopen_body_store();
        if let Err(error) =
            restarted_service.execute(&fixture.context, &mut restarted_store, &fixture.task)
        {
            let checkpoint_after_retry = fixture.kura.wsv_checkpoint(1);
            let manifest_after_retry = fixture.kura.commit_manifest(1);
            let replay_state_hash =
                crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref());
            panic!(
                "exact durable replay reuses the archived height: {error:?}; \
                     staged_state_hash={:?}; replay_state_hash={replay_state_hash:?}; \
                     checkpoint_after_retry={checkpoint_after_retry:?}; \
                     manifest_after_retry={manifest_after_retry:?}",
                staged_checkpoint.state_hash(),
            );
        }
        assert_eq!(restarted_state.committed_height(), 1);
        assert_eq!(
            archive
                .health_generation()
                .expect("read replayed archive generation"),
            generation_after_capture,
            "an exact replay must not publish a second archive generation"
        );
        let projection = archive
            .latest_at_or_before(&fixture.service.network_id, 1)
            .expect("read recovered projection")
            .expect("height one remains archived");
        assert_eq!(projection.key.height, 1);
        let qualification = archive
            .qualify_against_kura_tip(&fixture.service.network_id, fixture.kura.as_ref(), 0)
            .expect("recovered archive remains contiguous and exact");
        assert_eq!(qualification.activation_floor().height, 1);
        assert_eq!(qualification.archive_tip().height, 1);

        let (_, receipt) = restarted_service
            .kura
            .v2_finality_artifact_with_receipt(1)
            .expect("read exact finality receipt")
            .expect("height one finality receipt");
        let restarted_view = restarted_state.query_view();
        let outcome = archive
            .capture_kura_authenticated_view(&restarted_view, fixture.kura.as_ref(), &receipt)
            .expect("same exact committed view is replay-safe");
        assert_eq!(
            outcome,
            ReputationFinalizedArchiveInsertOutcome::ExactReplay
        );
    }
);

v2_apply_test!(
    reputation_archive_virtual_base_allows_commit_owned_successor_capture,
    {
        let mut fixture = ApplyFixture::new_with_reputation_archive();
        let archive_root = direct_archive_tempdir();
        let bounds =
            ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                .expect("valid retained-capture archive bounds");
        let archive = Arc::new(
            ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                .expect("open retained-capture archive"),
        );
        fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));

        let mut parent_store = fixture.reopen_body_store();
        fixture
            .execute(&mut parent_store)
            .expect("commit and archive the retention-floor block");
        drop(parent_store);
        let parent = archive
            .latest_at_or_before(&fixture.service.network_id, 1)
            .expect("read retention-floor projection")
            .expect("height-one archive anchor");
        let fence = archive
            .retention_fence_for(&parent.key)
            .expect("freeze exact authenticated retention fence");
        let retention_authority = ReputationRetentionAuthorityForTest::new();
        let retention_binding = retention_authority.binding();
        let proposal = archive
            .prepare_kura_authenticated_compaction(&fence, fixture.kura.as_ref())
            .expect("prepare exact Kura-authenticated checkpoint");
        let compaction = archive
            .approve_and_install_kura_authenticated_compaction(
                &proposal,
                fixture.kura.as_ref(),
                &retention_binding,
                &retention_authority,
            )
            .expect("approve and compact the Kura-authenticated height-one prefix");
        assert_eq!(compaction.retention_floor(), &parent.key);
        assert_eq!(
            compaction.generation(),
            fence.expected_generation() + 1,
            "checkpoint-head publication advances the archive generation"
        );
        assert_eq!(
            archive
                .retention_floor(&fixture.service.network_id)
                .expect("read active virtual base"),
            Some(parent.key.clone())
        );

        let mut successor = build_successor_apply_fixture(&fixture);
        let completion =
            match fixture
                .service
                .execute(&successor.context, &mut successor.store, &successor.task)
            {
                Ok(completion) => completion,
                Err(error) => {
                    assert!(
                        !matches!(
                            &error,
                            V2ApplyError::CommittedRecoveryRequired { stage, .. }
                                if *stage == "reputation finalized archive capture"
                        ),
                        "retained capture must not degrade to committed recovery: {error:?}"
                    );
                    panic!(
                        "commit-owned H+1 capture failed after virtual-base compaction: {error:?}"
                    );
                }
            };
        assert_eq!(completion.receipt().height(), 2);
        assert_eq!(fixture.state.committed_height(), 2);
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 2);
        assert_eq!(
            fixture
                .kura
                .get_durable_block_hash(NonZeroUsize::new(2).expect("height two")),
            Some(successor.body.hash())
        );
        let qualification = archive
            .qualify_against_kura_tip(&fixture.service.network_id, fixture.kura.as_ref(), 0)
            .expect("qualify retained successor against exact Kura tip");
        assert_eq!(qualification.activation_floor().height, 1);
        assert_eq!(qualification.archive_tip().height, 2);
        assert_eq!(
            qualification.checkpoint_digest(),
            Some(compaction.checkpoint_digest())
        );
        assert_eq!(qualification.kura_tip_height(), 2);
        assert_eq!(qualification.lag_blocks(), 0);
        let generation = archive
            .health_generation()
            .expect("read post-successor archive generation");

        drop(successor);
        fixture.service.reputation_finalized_archive = None;
        drop(archive);
        let reopened = ReputationFinalizedArchive::try_open_with_retention_authority(
            archive_root.path(),
            bounds,
            &fixture.service.network_id,
            fixture.kura.as_ref(),
            &retention_binding,
            &retention_authority,
        )
        .expect("reopen compacted successor archive through sealed authority");
        assert_eq!(
            reopened
                .health_generation()
                .expect("read reopened archive generation"),
            generation
        );
        assert_eq!(
            reopened
                .retention_floor(&fixture.service.network_id)
                .expect("read reopened virtual base"),
            Some(parent.key)
        );
        let reopened_qualification = reopened
            .qualify_against_kura_tip(&fixture.service.network_id, fixture.kura.as_ref(), 0)
            .expect("reopened archive preserves exact predecessor continuity");
        assert_eq!(reopened_qualification.archive_tip().height, 2);
        assert_eq!(
            reopened_qualification.checkpoint_digest(),
            Some(compaction.checkpoint_digest())
        );
        assert_eq!(reopened_qualification.kura_tip_height(), 2);

        let (_, receipt) = fixture
            .kura
            .v2_finality_artifact_with_receipt(2)
            .expect("read height-two finality receipt")
            .expect("height-two finality is durable");
        let view = fixture.state.query_view();
        assert_eq!(
            reopened
                .capture_kura_authenticated_view(&view, fixture.kura.as_ref(), &receipt)
                .expect("replay retained H+1 capture after reopen"),
            ReputationFinalizedArchiveInsertOutcome::ExactReplay
        );
        assert_eq!(
            reopened
                .health_generation()
                .expect("exact replay preserves archive generation"),
            generation
        );
    }
);

v2_apply_test!(
    reputation_retention_restart_recovers_both_cas_publication_boundaries,
    {
        for (failed_load_after_cas, checkpoint_was_published) in [(1, false), (3, true)] {
            let mut fixture = ApplyFixture::new_with_reputation_archive();
            let archive_root = direct_archive_tempdir();
            let bounds =
                ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                    .expect("valid retention-recovery archive bounds");
            let archive = Arc::new(
                ReputationFinalizedArchive::try_open(archive_root.path(), bounds)
                    .expect("open retention-recovery archive"),
            );
            fixture.service.reputation_finalized_archive = Some(Arc::clone(&archive));

            let mut parent_store = fixture.reopen_body_store();
            fixture
                .execute(&mut parent_store)
                .expect("commit and archive the retention-floor block");
            drop(parent_store);
            let parent = archive
                .latest_at_or_before(&fixture.service.network_id, 1)
                .expect("read retention-floor projection")
                .expect("height-one archive anchor");
            let fence = archive
                .retention_fence_for(&parent.key)
                .expect("freeze exact retention fence");
            let authority = ReputationRetentionAuthorityForTest::new();
            let binding = authority.binding();
            let proposal = archive
                .prepare_kura_authenticated_compaction(&fence, fixture.kura.as_ref())
                .expect("prepare exact retention proposal");
            authority.fail_nth_load_after_next_cas(failed_load_after_cas);

            assert!(matches!(
                archive.approve_and_install_kura_authenticated_compaction(
                    &proposal,
                    fixture.kura.as_ref(),
                    &binding,
                    &authority,
                ),
                Err(ReputationFinalizedArchiveError::RetentionAuthorityCasAmbiguous)
            ));
            assert_eq!(
                archive
                    .retention_floor(&fixture.service.network_id)
                    .expect("read ambiguous local retention floor"),
                checkpoint_was_published.then(|| parent.key.clone())
            );
            assert_eq!(
                std::fs::read_dir(archive.root().join("anchors"))
                    .expect("read ambiguous anchor namespace")
                    .count(),
                1,
                "no physical anchor may be unlinked before the post-publication readback"
            );
            assert_eq!(
                std::fs::read_dir(archive.root().join("checkpoints"))
                    .expect("read ambiguous checkpoint namespace")
                    .count(),
                usize::from(checkpoint_was_published)
            );

            fixture.service.reputation_finalized_archive = None;
            drop(archive);
            let recovered = ReputationFinalizedArchive::try_open_with_retention_authority(
                archive_root.path(),
                bounds,
                &fixture.service.network_id,
                fixture.kura.as_ref(),
                &binding,
                &authority,
            )
            .expect("recover exact externally approved retention state");
            assert_eq!(
                recovered
                    .retention_floor(&fixture.service.network_id)
                    .expect("read recovered retention floor"),
                Some(parent.key.clone())
            );
            assert_eq!(
                std::fs::read_dir(recovered.root().join("anchors"))
                    .expect("read recovered anchor namespace")
                    .count(),
                0,
                "recovery must clean the exact approved physical prefix"
            );
            assert_eq!(
                std::fs::read_dir(recovered.root().join("checkpoints"))
                    .expect("read recovered checkpoint namespace")
                    .count(),
                1
            );
            let recovered_generation = recovered
                .health_generation()
                .expect("read recovered archive generation");

            drop(recovered);
            let reopened = ReputationFinalizedArchive::try_open_with_retention_authority(
                archive_root.path(),
                bounds,
                &fixture.service.network_id,
                fixture.kura.as_ref(),
                &binding,
                &authority,
            )
            .expect("repeat exact retention recovery");
            assert_eq!(
                reopened
                    .health_generation()
                    .expect("read replayed archive generation"),
                recovered_generation,
                "recovery replay must be deterministic"
            );
            assert_eq!(
                reopened
                    .retention_floor(&fixture.service.network_id)
                    .expect("read replayed retention floor"),
                Some(parent.key)
            );
        }
    }
);

v2_apply_test!(
    crash_after_staged_checkpoint_replays_exact_tip_without_double_apply,
    {
        let fixture = ApplyFixture::new();
        let mut first_process_store = fixture.reopen_body_store();
        fixture.service.fail_after_wsv_checkpoint_for_test();
        let first_error = fixture
            .service
            .execute(&fixture.context, &mut first_process_store, &fixture.task)
            .expect_err("inject crash after checkpoint fsync and before WSV publication");
        assert!(matches!(
            &first_error,
            V2ApplyError::InjectedCrashAfterWsvCheckpoint
        ));
        assert!(first_error.requires_restart_recovery());
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        let staged_checkpoint = fixture
            .kura
            .wsv_checkpoint(1)
            .expect("read staged checkpoint")
            .expect("checkpoint must be durable before WSV publication");
        assert!(
            fixture
                .kura
                .commit_manifest(1)
                .expect("read absent manifest")
                .is_none(),
            "the pre-WSV checkpoint must remain unbound until State commits"
        );
        assert_eq!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read pre-WSV finality")
                .expect("finality must be durable before WSV publication")
                .block_hash,
            fixture.body.hash()
        );
        let staged_state_hash = staged_checkpoint.state_hash();
        drop(first_process_store);

        // Snapshot publication is gated on the complete
        // checkpoint/manifest/finality tuple, so a process crash reloads
        // the last finalized snapshot (height zero here). The exact
        // durable checkpoint authenticates the overlay replay before live
        // State can cross its commit boundary.
        let (restarted_service, restarted_state) =
            fixture.restart_service_from_last_finalized_snapshot();
        assert_eq!(restarted_state.committed_height(), 0);
        let mut restarted_store = fixture.reopen_body_store();
        restarted_service
            .execute(&fixture.context, &mut restarted_store, &fixture.task)
            .expect("authenticated WAL/body retry reapplies the sole Kura tip");
        assert_eq!(restarted_state.committed_height(), 1);
        let first_artifact = fixture
            .kura
            .v2_finality_artifact(1)
            .expect("read recovered finality")
            .expect("recovery publishes finality");
        assert_eq!(first_artifact.block_hash, fixture.body.hash());
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref()),
            staged_state_hash,
            "recovery must reproduce the exact pre-commit checkpointed WSV"
        );

        let durable_state_hash =
            crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref());
        restarted_service
            .execute(&fixture.context, &mut restarted_store, &fixture.task)
            .expect("an exact post-finality retry is idempotent");
        assert_eq!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read repeated finality")
                .as_ref(),
            Some(&first_artifact)
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(restarted_state.as_ref()),
            durable_state_hash,
            "idempotent retry must not execute the block twice"
        );
        fixture.assert_complete_for_state(restarted_state.as_ref());
    }
);

v2_apply_test!(restart_recovers_manifest_after_pre_wsv_finality, {
    let fixture = ApplyFixture::new();
    let mut store = fixture.reopen_body_store();
    fixture.kura.fail_next_commit_manifest_write_for_tests();
    let error = fixture
        .execute(&mut store)
        .expect_err("manifest failure follows the irreversible commit boundary");
    assert!(
        matches!(
            &error,
            V2ApplyError::CommittedRecoveryRequired { stage, .. }
                if *stage == "post-apply metadata"
        ),
        "unexpected committed recovery classification: {error:?}"
    );
    assert!(error.requires_restart_recovery());
    assert_eq!(fixture.state.committed_height(), 1);
    assert!(
        fixture
            .kura
            .wsv_checkpoint(1)
            .expect("read checkpoint")
            .is_some()
    );
    assert!(
        fixture
            .kura
            .commit_manifest(1)
            .expect("read manifest")
            .is_none()
    );
    assert!(
        fixture
            .kura
            .v2_finality_artifact(1)
            .expect("read finality")
            .is_some()
    );

    drop(store);
    let mut reopened = fixture.reopen_body_store();
    fixture.execute(&mut reopened).expect("complete manifest");
    fixture.assert_complete();
});

v2_apply_test!(restart_recovers_kura_block_before_pre_wsv_finality, {
    let fixture = ApplyFixture::new();
    let mut store = fixture.reopen_body_store();
    fixture.kura.fail_next_v2_finality_write_for_tests();
    let error = fixture
        .execute(&mut store)
        .expect_err("finality failure follows the irreversible commit boundary");
    assert!(
        matches!(
            &error,
            V2ApplyError::CommittedRecoveryRequired { stage, .. }
                if *stage == "pre-WSV v2 finality artifact"
        ),
        "unexpected committed recovery classification: {error:?}"
    );
    assert!(error.requires_restart_recovery());
    assert_eq!(fixture.state.committed_height(), 0);
    assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
    assert!(
        fixture
            .kura
            .wsv_checkpoint(1)
            .expect("read checkpoint")
            .is_none()
    );
    assert!(
        fixture
            .kura
            .commit_manifest(1)
            .expect("read manifest")
            .is_none()
    );
    assert!(
        fixture
            .kura
            .v2_finality_artifact(1)
            .expect("read finality")
            .is_none()
    );

    drop(store);
    let mut reopened = fixture.reopen_body_store();
    fixture
        .execute(&mut reopened)
        .expect("complete pre-WSV finality and apply");
    fixture.assert_complete();
});

v2_apply_test!(
    complete_apply_replay_is_idempotent_and_never_advances_twice,
    {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.execute(&mut store).expect("initial apply");
        let state_hash = crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref());
        let artifact = fixture
            .kura
            .v2_finality_artifact(1)
            .expect("read finality")
            .expect("finality exists");

        fixture.execute(&mut store).expect("idempotent replay");
        fixture.assert_complete();
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()),
            state_hash
        );
        assert_eq!(
            fixture
                .kura
                .v2_finality_artifact(1)
                .expect("read repeated finality"),
            Some(artifact)
        );
    }
);
