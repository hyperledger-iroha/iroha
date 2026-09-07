/// Advance the same autonomous lane through a second real public source and merge.
///
/// The first source remains at global height two with its application at three.
/// Source four advances the exact lane predecessor and applies at five, allowing
/// context six to exercise both non-genesis terminal replay and an older receipt.
fn second_autonomous_cycle_preserves_terminal_replay(
    fixture: &ApplyFixture,
    original_certificate: iroha_data_model::block::consensus::LaneBlockCertificateV1,
    original_votes: [&[crate::lane_consensus::LaneBlockVoteV1]; 2],
    validator_keys: &[KeyPair],
    local_key: &KeyPair,
    limits: crate::sumeragi::v2_lane_work::V2LaneWorkLimits,
) {
    use crate::state::WorldReadOnly as _;

    assert_eq!(fixture.state.committed_height(), 3);
    let original = &original_certificate.proposal;
    assert_eq!(original.descriptor.proposal_height, 2);
    assert_eq!(original.descriptor.lane_block_height, 1);
    let original_source_height = NonZeroUsize::new(2).expect("original source height");
    let original_source_wire = fixture
        .kura
        .get_block(original_source_height)
        .expect("retain original public source")
        .encode_wire()
        .expect("encode original public source");
    let original_finality = fixture
        .kura
        .v2_finality_artifact(2)
        .expect("read original public source finality")
        .expect("original source remains finalized");
    let original_receipt = fixture
        .kura
        .read_lane_block_application_receipt(
            original.descriptor.lane_id,
            original.descriptor.lane_block_height,
        )
        .expect("first real economic application has its exact receipt");
    assert_eq!(original_receipt.proposal, *original);
    assert_eq!(original_receipt.application_block_height, 3);
    let original_ledger = fixture.state.merge_ledger.snapshot();
    assert_eq!(original_ledger.len(), 1);

    let context_four = verified_successor_context_at_fixture_tip(fixture);
    assert_eq!(context_four.context().height, 4);
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
    let queue = fixture_queue(fixture.state.as_ref(), events_sender.clone());
    let journals = tempfile::tempdir().expect("second autonomous cycle journals");
    let plans_path = journals.path().join("plans.norito");
    let reservations_path = journals.path().join("reservations.norito");
    queue
        .install_plan_journal(&plans_path, 1024 * 1024, true)
        .expect("install second-cycle QueuePlan journal");
    queue
        .install_lane_reservation_journal(&reservations_path, 1024 * 1024)
        .expect("install second-cycle reservation journal");
    let asset = AssetId::new(
        fixture_reserve_asset_definition(),
        fixture.service.genesis_account.clone(),
    );
    let balance = || {
        let view = fixture.state.view();
        view.world
            .asset(&asset)
            .ok()
            .map(|asset| (**asset.value()).clone())
    };
    assert_eq!(
        balance(),
        Some(iroha_primitives::numeric::Quantity::from(2_u32))
    );
    let mint_asset = asset.clone();
    let (payload, entrypoints) = reserve_canonical_autonomous_batch_at_context_with_instructions(
        fixture,
        &queue,
        context_four.context(),
        1,
        move |_| {
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                mint_asset.clone(),
            ))]
        },
        false,
        None,
    );
    let descriptor = payload.origin_proposal.descriptor.clone();
    assert_eq!(descriptor.proposal_height, 4);
    assert_eq!(descriptor.lane_id, original.descriptor.lane_id);
    assert_eq!(descriptor.dataspace_id, original.descriptor.dataspace_id);
    assert_eq!(
        descriptor.lane_incarnation,
        original.descriptor.lane_incarnation
    );
    assert_eq!(descriptor.lane_block_height, 2);
    assert_eq!(descriptor.previous_lane_block_height, 1);
    assert_eq!(
        descriptor.previous_lane_block_descriptor_hash,
        Some(original.descriptor.descriptor_hash)
    );
    assert!(payload.native_amx_receipts.iter().all(Option::is_none));
    assert!(
        fixture
            .state
            .certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
                &payload.origin_proposal
            )
    );
    let reservation_keys = payload.reservation_keys.clone();
    let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
        &payload,
        payload.network_id,
        payload.epoch,
    )
    .expect("encode second canonical autonomous payload");
    let mut source = build_apply_fixture_at_context_with_autonomous_payloads(
        fixture,
        context_four.context().clone(),
        vec![envelope],
    );
    fixture
        .service
        .execute(&source.context, &mut source.store, &source.task)
        .expect("apply actual public source carrier four");
    assert_eq!(fixture.state.committed_height(), 4);
    assert_eq!(
        balance(),
        Some(iroha_primitives::numeric::Quantity::from(2_u32))
    );
    assert!(
        entrypoints
            .iter()
            .all(|hash| !fixture.state.has_committed_entrypoint(*hash))
    );
    let context_five = verified_successor_context_at_fixture_tip(fixture);
    assert_eq!(context_five.context().height, 5);

    drop(queue);
    let queue = fixture_queue(fixture.state.as_ref(), events_sender.clone());
    let replay = queue
        .install_lane_reservation_journal(&reservations_path, 1024 * 1024)
        .expect("reopen second-cycle reservation ownership");
    assert_eq!(replay.restored, reservation_keys.len());
    queue
        .install_plan_journal(&plans_path, 1024 * 1024, true)
        .expect("reopen second-cycle QueuePlan journal");
    queue
        .replay_plan_journal(fixture.state.as_ref())
        .expect("replay second-cycle executable reservations");
    let planning = plan_lane_reservation_ownership(
        fixture.state.as_ref(),
        queue.as_ref(),
        fixture.kura.as_ref(),
        &context_five,
        None,
    )
    .expect("resolve second historical public source from canonical authority");
    let LaneReservationReconciliationPlanning::InstallHistoricalAutonomousRecoveries(installs) =
        planning
    else {
        panic!("second public source must require its exact historical installation");
    };
    assert_eq!(installs.len(), 1);
    let install = installs
        .into_iter()
        .next()
        .expect("second historical installation");
    assert!(install.has_valid_identity());
    assert_eq!(install.historical_context, source.context);
    assert_eq!(install.canonical_body.height, 4);
    assert_eq!(install.canonical_body.block_hash, source.body.hash());
    assert_eq!(install.payload.origin_proposal.descriptor, descriptor);
    assert_eq!(install.payload.entrypoints, payload.entrypoints);
    assert_eq!(install.payload.reservation_keys, reservation_keys);
    assert_eq!(
        install_historical_autonomous_lane_recovery(
            fixture.state.as_ref(),
            fixture.kura.as_ref(),
            &install
        )
        .expect("persist exact second historical execution input"),
        HistoricalAutonomousLaneRecoveryInstallOutcome::Installed
    );
    let planning = plan_lane_reservation_ownership(
        fixture.state.as_ref(),
        queue.as_ref(),
        fixture.kura.as_ref(),
        &context_five,
        None,
    )
    .expect("replan installed second historical source");
    let LaneReservationReconciliationPlanning::Ready(plan) = planning else {
        panic!("installed second source must make ownership reconciliation ready");
    };
    let summary = apply_lane_reservation_reconciliation_plan(
        fixture.state.as_ref(),
        queue.as_ref(),
        fixture.kura.as_ref(),
        plan,
    )
    .expect("publish second historical reservation ownership");
    assert_eq!(summary.recovered, reservation_keys.len());
    assert_eq!(summary.retained_historical_recovery, reservation_keys.len());
    assert!(!queue.lane_reservation_startup_reconciliation_pending());

    let local_peer = PeerId::new(local_key.public_key().clone());
    assert!(descriptor.validator_set.contains(&local_peer));
    let mut lane_work = crate::sumeragi::v2_lane_work::V2LaneWorkAdapter::new(
        context_five.context().clone(),
        local_peer.clone(),
        local_key.clone(),
        true,
        Arc::clone(&fixture.state),
        Arc::clone(&fixture.kura),
        limits,
        None,
    )
    .expect("hydrate second historical source in real lane work");
    let generation = fixture
        .kura
        .claim_autonomous_lifecycle_process_generation(install.payload.network_id, &local_peer)
        .expect("claim second-cycle lifecycle process generation");
    let _lifecycle_group = install_live_lifecycle_cursor_for_apply_test(
        fixture.kura.as_ref(),
        &generation,
        &install.payload,
        install.historical_context_id,
        &local_peer,
        local_key,
    );
    let (certificate, prepare_votes, commit_votes) =
        terminal_cycle_certificate(fixture, &install.payload, validator_keys);
    let retained = crate::sumeragi::v2_lane_work::tests::retain_public_lane_evidence_before_application_for_test(
        Arc::clone(&fixture.state), Arc::clone(&fixture.kura), context_five.context().clone(),
        limits, &certificate, &commit_votes[0],
    );
    assert_eq!(
        lane_work.accept_lane_message(
            crate::sumeragi::InboundBlockMessage::from_authenticated_peer(
                crate::sumeragi::message::BlockMessage::LaneBlockCertificate(Box::new(
                    certificate.clone()
                )),
                PeerId::new(validator_keys[0].public_key().clone()),
            ),
            0,
        ),
        crate::sumeragi::v2_lane_work::V2LaneIngressOutcome::Inserted
    );
    assert!(matches!(
        lane_work
            .service_next_historical_recovery()
            .expect("publish second certified execution bundle"),
        crate::sumeragi::v2_lane_work::HistoricalRecoveryServiceOutcome::Complete(_)
    ));
    let application_header = lane_work
        .merge_carrier_context_header(0)
        .expect("derive exact merge five application header");
    let candidate = fixture
        .state
        .build_merge_execution_candidate(application_header.clone(), context_five.context().mode)
        .expect("select second source from the actually applied predecessor frontier");
    assert_eq!(candidate.carrier_height, 5);
    assert_eq!(candidate.carrier_parent_hash, source.body.hash());
    assert_eq!(candidate.epoch_id, original_ledger[0].epoch_id + 1);
    let batch = candidate
        .execution_batch
        .as_ref()
        .expect("second autonomous execution batch");
    assert_eq!(batch.lanes.len(), 1);
    assert_eq!(batch.lanes[0].proposal, certificate.proposal);
    assert_eq!(
        batch.lanes[0].entrypoint_hashes,
        install.payload.entrypoint_hashes
    );
    assert!(batch.lanes[0].results.iter().all(|result| result.is_ok()));
    let entry = terminal_cycle_merge_entry(&candidate, context_five.context(), validator_keys);
    fixture
        .state
        .validate_certified_merge_entry_for_global_order(&entry, context_five.context().mode)
        .expect("revalidate second certified merge against actual WSV");
    let entry_hash = fixture
        .kura
        .persist_pending_certified_merge_entry(&entry)
        .expect("persist exact second certified merge sidecar");
    assert_eq!(entry_hash, crate::merge::merge_ledger_entry_hash(&entry));
    let service = V2ApplyService::new(
        Arc::clone(&fixture.state),
        Arc::clone(&queue),
        Arc::clone(&fixture.kura),
        None,
        None,
        fixture.service.block_cadence,
        fixture.service.genesis_account.clone(),
        events_sender,
        fixture.service.validator_set_pops.clone(),
    );
    let mut merge = terminal_cycle_merge_apply_fixture(
        fixture,
        &service,
        context_five.context(),
        &entry,
        &application_header,
        validator_keys,
    );
    service
        .execute(&merge.context, &mut merge.store, &merge.task)
        .expect("economically apply the actual merge five carrier");
    assert_eq!(fixture.state.committed_height(), 5);
    assert_eq!(
        balance(),
        Some(iroha_primitives::numeric::Quantity::from(3_u32))
    );
    assert!(
        entrypoints
            .iter()
            .all(|hash| fixture.state.has_committed_entrypoint(*hash))
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert!(queue.lane_reservation_release_barriers().is_empty());
    assert!(queue.lane_reservation_group_is_finalized_for_diagnostics(&reservation_keys));
    let ledger = fixture.state.merge_ledger.snapshot();
    assert_eq!(ledger.len(), 2);
    assert_eq!(ledger[0], original_ledger[0]);
    assert_eq!(ledger[1], Arc::new(entry));
    let receipt = fixture
        .kura
        .read_lane_block_application_receipt(descriptor.lane_id, descriptor.lane_block_height)
        .expect("second merge writes its genuine economic receipt");
    assert_eq!(receipt.proposal, certificate.proposal);
    assert_eq!(
        receipt.format,
        crate::kura::LaneBlockApplicationReceiptArtifactFormat::MergeExecution
    );
    assert_eq!(receipt.application_block_height, 5);
    assert_eq!(receipt.application_block_hash, merge.body.hash());
    assert!(
        fixture
            .state
            .certified_autonomous_lane_block_is_globally_applied_cached(&certificate.proposal)
    );
    assert!(
        !fixture
            .state
            .certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
                &certificate.proposal
            )
    );
    assert!(
        fixture
            .state
            .certified_autonomous_lane_block_is_globally_applied_cached(original)
    );
    assert!(
        fixture
            .kura
            .autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair(original),
        "the older exact receipt must revalidate after the real frontier advances"
    );
    for change_incarnation in [false, true] {
        let mut wrong = original.clone();
        if change_incarnation {
            wrong.descriptor.lane_incarnation = Hash::new(b"another autonomous incarnation");
        } else {
            wrong.descriptor.subject_hash = Hash::new(b"another autonomous descriptor subject");
        }
        wrong.descriptor.descriptor_hash = wrong.descriptor.computed_descriptor_hash();
        wrong.proposal_hash = wrong.computed_proposal_hash();
        crate::lane_consensus::validate_lane_block_proposal(&wrong)
            .expect("the negative uses a structurally valid distinct identity");
        assert_ne!(
            wrong.descriptor.descriptor_hash,
            original.descriptor.descriptor_hash
        );
        assert!(
            !fixture
                .kura
                .autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair(&wrong)
        );
        assert!(
            !fixture
                .state
                .certified_autonomous_lane_block_is_globally_applied_cached(&wrong)
        );
    }
    let context_six = verified_successor_context_at_fixture_tip(fixture);
    assert_eq!(context_six.context().height, 6);
    crate::sumeragi::v2_lane_work::tests::inspect_applied_public_lane_qc_replay_for_test(
        Arc::clone(&fixture.state),
        Arc::clone(&fixture.kura),
        context_six.context().clone(),
        limits,
        certificate,
        [prepare_votes.as_slice(), commit_votes.as_slice()],
        validator_keys,
        local_key,
        retained,
    );
    assert_eq!(
        fixture
            .kura
            .get_block(original_source_height)
            .expect("original source after second merge")
            .encode_wire()
            .expect("encode retained original source"),
        original_source_wire
    );
    assert_eq!(
        fixture
            .kura
            .v2_finality_artifact(2)
            .expect("original finality after second merge"),
        Some(original_finality)
    );
    assert_eq!(
        fixture.kura.read_lane_block_application_receipt(
            original.descriptor.lane_id,
            original.descriptor.lane_block_height
        ),
        Some(original_receipt)
    );
    crate::sumeragi::v2_lane_work::tests::inspect_applied_public_lane_qc_replay_for_test(
        Arc::clone(&fixture.state),
        Arc::clone(&fixture.kura),
        context_six.context().clone(),
        limits,
        original_certificate,
        original_votes,
        validator_keys,
        local_key,
        Vec::new(),
    );
    assert_eq!(
        balance(),
        Some(iroha_primitives::numeric::Quantity::from(3_u32))
    );
}

/// Sign exact three-of-four lane QCs with the canonical executable READY body.
fn terminal_cycle_certificate(
    fixture: &ApplyFixture,
    payload: &LaneExecutablePayloadV1,
    keys: &[KeyPair],
) -> (
    iroha_data_model::block::consensus::LaneBlockCertificateV1,
    Vec<crate::lane_consensus::LaneBlockVoteV1>,
    Vec<crate::lane_consensus::LaneBlockVoteV1>,
) {
    let proposal = &payload.origin_proposal;
    assert_eq!(proposal.descriptor.validator_count, 4);
    assert_eq!(proposal.descriptor.min_quorum, 3);
    assert_eq!(
        proposal.descriptor.validator_set,
        keys.iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>()
    );
    let availability = crate::lane_consensus::lane_payload_availability_body(
        payload,
        proposal,
        payload.network_id,
        payload.epoch,
    )
    .expect("exact second-cycle executable availability");
    let votes = |phase| {
        keys[..3]
            .iter()
            .map(|key| {
                let body = proposal.vote_body(phase);
                let signer = PeerId::new(key.public_key().clone());
                let ready = (phase == CertPhase::Prepare).then(|| {
                    crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                        availability.clone(),
                        signer.clone(),
                        fixture.service.validator_set_pops.clone(),
                        key.private_key(),
                    )
                    .expect("sign exact second-cycle READY")
                });
                crate::lane_consensus::LaneBlockVoteV1 {
                    bls_signature: Signature::try_new(
                        key.private_key(),
                        &body.signature_preimage(),
                    )
                    .expect("sign exact second-cycle lane vote")
                    .payload()
                    .to_vec(),
                    body,
                    signer,
                    payload_availability_vote: ready,
                }
            })
            .collect::<Vec<_>>()
    };
    let prepare_votes = votes(CertPhase::Prepare);
    let commit_votes = votes(CertPhase::Commit);
    let qc = |phase, votes: &[crate::lane_consensus::LaneBlockVoteV1]| {
        crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            proposal.vote_body(phase),
            proposal.descriptor.validator_set.clone(),
            votes,
        )
        .expect("aggregate exactly three of four lane votes")
    };
    (
        iroha_data_model::block::consensus::LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: qc(CertPhase::Prepare, &prepare_votes),
            commit_qc: qc(CertPhase::Commit, &commit_votes),
        },
        prepare_votes,
        commit_votes,
    )
}

/// Authenticate a real State-built merge candidate with three of four global voters.
fn terminal_cycle_merge_entry(
    candidate: &crate::merge::MergeLedgerCandidate,
    context: &wire::HeightContext,
    keys: &[KeyPair],
) -> MergeLedgerEntry {
    let validators = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    assert_eq!(validators.len(), 4);
    assert_eq!(
        validators,
        keys.iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>()
    );
    let validator_hash = HashOf::new(&validators);
    let digest = crate::merge::merge_qc_message_digest(
        &context.network_id,
        candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_hash,
    );
    let signatures = keys[..3]
        .iter()
        .map(|key| {
            Signature::try_new(key.private_key(), digest.as_ref())
                .expect("sign second merge vote")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let proofs = keys[..3]
        .iter()
        .enumerate()
        .map(|(index, key)| iroha_data_model::merge::MergeSignerProof {
            signer: u32::try_from(index).expect("merge signer index"),
            proof_of_possession: iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("second merge signer PoP"),
        })
        .collect::<Vec<_>>();
    let qc = MergeQuorumCertificate::new(
        candidate.view,
        candidate.epoch_id,
        candidate.carrier_height,
        candidate.carrier_parent_hash,
        context.network_id,
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_hash,
        validators,
        vec![0b0000_0111],
        proofs,
        iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate second merge three-of-four signatures"),
        digest,
    );
    let entry = candidate.clone().into_entry(qc);
    crate::sumeragi::v2_lane_work::authenticate_merge_entry_for_height_context(context, &entry)
        .expect("authenticate second merge against the exact frozen context");
    entry
}

/// Build a canonical merge carrier and real global Commit proof for the current tip.
fn terminal_cycle_merge_apply_fixture(
    fixture: &ApplyFixture,
    service: &V2ApplyService,
    context: &wire::HeightContext,
    entry: &MergeLedgerEntry,
    header: &BlockHeader,
    keys: &[KeyPair],
) -> SuccessorApplyFixture {
    let height = context.height;
    assert_eq!(header.height().get(), height);
    let parent_height =
        NonZeroUsize::new(usize::try_from(height - 1).expect("merge parent height"))
            .expect("nonzero merge parent");
    let parent = fixture
        .kura
        .get_block(parent_height)
        .expect("read actual merge parent");
    assert_eq!(fixture.state.latest_block_hash_fast(), Some(parent.hash()));
    assert_eq!(header.prev_block_hash(), Some(parent.hash()));
    let (_, time) = TimeSource::new_mock(header.creation_time());
    let confidential_features = {
        let view = fixture.state.view();
        let digest = crate::state::compute_confidential_feature_digest(
            view.world(),
            &view.zk,
            view.sccp_registry.as_ref(),
            height,
        );
        (!digest.is_empty()).then_some(digest)
    };
    let leader = context.leader(0);
    let body = BlockBuilder::new_with_time_source(Vec::new(), time)
        .chain(0, Some(parent.as_ref()))
        .bind_certified_merge_application_context(header)
        .expect("bind second certified application context")
        .with_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
            &fixture.state.nexus_snapshot(),
            height,
        )))
        .with_confidential_features(confidential_features)
        .with_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_merge_entry(CertifiedMergeLedgerReference::new(entry)),
        ))
        .try_sign_with_index(
            keys[usize::try_from(leader).expect("merge leader")].private_key(),
            u64::from(leader),
        )
        .expect("sign second merge carrier")
        .unpack(|_| {});
    let body = SignedBlock::from(body);
    assert_eq!(body.header().height().get(), height);
    let wire_bytes = body.encode_wire().expect("canonical second merge wire");
    let subject = wire::BlockSubject {
        parent_block_hash: Some(parent.hash()),
        block_hash: body.hash(),
        payload_hash: Hash::new(&wire_bytes),
    };
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height,
        view: 0,
    };
    let manifest = crate::sumeragi::v2_chunks::encode_payload(context, round, subject, &wire_bytes)
        .expect("second merge signed RS16 manifest")
        .into_parts()
        .0;
    let execution_commitment = service
        .validate_candidate(context, &body)
        .expect("re-execute the second certified batch before global Commit");
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = keys[..3]
        .iter()
        .map(|key| {
            Signature::try_new(key.private_key(), &preimage)
                .expect("sign second global Commit vote")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate second global three-of-four Commit"),
    };
    let body_root = tempfile::tempdir().expect("second merge canonical body store");
    let mut store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open second merge body store");
    let durable = store
        .store(manifest, wire_bytes)
        .expect("persist exact second merge body");
    let validated = store
        .validate(&durable, |candidate| {
            service.validate_candidate(context, candidate)
        })
        .expect("persist actual second merge validation receipt");
    let task = ApplyTask::for_test(
        height,
        EventTag::new(height, 0, Generation::new(height)),
        subject,
        certificate,
        validated,
    );
    SuccessorApplyFixture {
        context: context.clone(),
        body,
        task,
        _body_root: body_root,
        store,
    }
}
