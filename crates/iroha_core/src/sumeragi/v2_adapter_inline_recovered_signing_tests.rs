#[cfg(feature = "bls")]
#[test]
fn recovered_proposal_broadcast_and_sign_seals_exact_wal_body_and_successor() {
    let directory = TempDir::new().expect("temporary recovered FIFO directory");
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let local = context.leader(round.view);
    let body_subject = subject(0xD6);
    let outbound = encode_payload(&context, round, body_subject, b"recovered proposal FIFO")
        .expect("encode recovered proposal payload");
    let manifest = outbound.manifest().clone();
    let proposal = wire::Proposal {
        round,
        proposer: local,
        subject: body_subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    let (_, validated) = validated_receipts_for_manifest(&context, &proposal.manifest);
    let body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        directory.path().join("next-vote-body-owner"),
        context.clone(),
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open exact next-Vote body-store owner");
    let body_store_identity = body_store.instance_identity();
    let prepare = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: body_subject,
        execution_commitment: validated.execution_commitment(),
        signer: local,
        signature: Vec::new(),
    };
    let startup = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xD6; 32],
        vec![
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::PrepareIntent(prepare.clone()),
        ],
    );
    let RecoveredAdapterStartup {
        mut adapter,
        effects,
    } = startup;
    let [AdapterEffect::Sign { tag, request }] = effects.as_slice() else {
        panic!("recovered Proposal/Prepare FIFO must expose the Proposal Sign first")
    };
    assert_eq!(request, &SignRequest::Proposal(proposal.clone()));
    let tag = *tag;
    let request = request.clone();
    let proposal_identity = adapter
        .authenticate_recovered_wal_frame(&adapter.wal.recovered_records()[0])
        .expect("authenticate ProposalIntent frame")
        .0;
    let prepare_identity = adapter
        .authenticate_recovered_wal_frame(&adapter.wal.recovered_records()[1])
        .expect("authenticate PrepareIntent frame")
        .0;
    let local_index = usize::try_from(local).expect("fixture signer index fits usize");
    let signature = Signature::new(
        keys[local_index].private_key(),
        &request.signature_preimage(),
    )
    .payload()
    .to_vec();
    let completion =
        super::super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
            1,
            tag,
            request,
            signature,
            Some(outbound),
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1::ControlProposal,
        );
    let mut preview = adapter
        .prepare_recovered_lifecycle_sign_completion(completion)
        .expect("preview exact recovered Proposal signature");
    assert_eq!(
        preview.shape(),
        RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
    );
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign)
    );
    let broadcast = preview.broadcast_effect().clone();
    let next_sign = preview
        .next_sign_effect()
        .expect("recovered FIFO retains its Prepare Sign")
        .clone();
    assert!(matches!(
        &next_sign,
        AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        } if vote == &prepare
    ));

    let mut late_prepare_sign = next_sign.clone();
    let AdapterEffect::Sign { tag, .. } = &mut late_prepare_sign else {
        unreachable!("fixture successor is a Vote Sign")
    };
    *tag = reducer::EventTag::new(
        round.height,
        round.view.saturating_add(1),
        reducer::Generation::new(round.height),
    );
    assert!(
        RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            broadcast.clone(),
            late_prepare_sign,
        )
        .is_none(),
        "a Prepare successor cannot move to a later EventTag view"
    );

    let mut signed_prepare = prepare.clone();
    signed_prepare.signature = Signature::new(
        keys[local_index].private_key(),
        &SignRequest::Vote(prepare.clone()).signature_preimage(),
    )
    .payload()
    .to_vec();
    let prepare_broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(signed_prepare),
    ));
    let mut commit = prepare.clone();
    commit.phase = wire::GlobalPhase::Commit;
    let later_commit_sign = AdapterEffect::Sign {
        tag: reducer::EventTag::new(
            round.height,
            round.view.saturating_add(1),
            reducer::Generation::new(round.height),
        ),
        request: SignRequest::Vote(commit),
    };
    let proposal_broadcast = preview.broadcast.clone();
    let proposal_next_sign = preview.next_sign.clone();
    preview.broadcast = prepare_broadcast.clone();
    preview.next_sign = Some(later_commit_sign.clone());
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign)
    );
    let Some(AdapterEffect::Sign {
        request: SignRequest::Vote(malformed_commit),
        ..
    }) = preview.next_sign.as_mut()
    else {
        unreachable!("fixture successor remains a Vote Sign")
    };
    malformed_commit.subject = subject(0xD7);
    assert_eq!(
        preview.settlement_family(),
        None,
        "a mismatched combined relation has no settlement family"
    );
    preview.broadcast = proposal_broadcast;
    preview.next_sign = proposal_next_sign;
    assert!(
        RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            prepare_broadcast,
            later_commit_sign,
        )
        .is_some(),
        "a Commit successor may retain a later EventTag view than its vote round"
    );

    let expected_manifest_hash = Some(HashOf::new(&manifest));
    let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
    let exact_body_lookup = preview
        .project_broadcast_and_sign_body_lookup_for_test(
            body_store_identity.clone(),
            Arc::clone(&output_guard),
        )
        .expect("bind exact reducer body lookup to the preview/store owner");

    let foreign_durable =
        DurableBodyReceipt::for_test(context.id(), round, subject(0xD7), HashOf::new(&manifest));
    let foreign_body = ValidatedBodyReceipt::for_test(foreign_durable);
    assert!(
        RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
            RecoveredLifecycleNextVoteBodyLookupV1::for_test(&prepare, expected_manifest_hash,)
                .expect("project exact reducer body lookup"),
            foreign_body.clone(),
            body_store_identity.clone(),
        )
        .is_none(),
        "a substituted validated body cannot mint exact-owner authority"
    );
    let mut foreign_manifest = manifest.clone();
    foreign_manifest.payload_size_bytes = foreign_manifest
        .payload_size_bytes
        .checked_add(1)
        .expect("fixture payload length has headroom");
    let foreign_manifest_durable = DurableBodyReceipt::for_test(
        context.id(),
        round,
        body_subject,
        HashOf::new(&foreign_manifest),
    );
    let foreign_manifest_body = ValidatedBodyReceipt::for_test_with_commitment(
        foreign_manifest_durable,
        validated.execution_commitment(),
    );
    assert!(
        RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
            RecoveredLifecycleNextVoteBodyLookupV1::for_test(&prepare, expected_manifest_hash,)
                .expect("project exact reducer body lookup"),
            foreign_manifest_body,
            body_store_identity.clone(),
        )
        .is_none(),
        "same-coordinate foreign manifest cannot mint exact-owner authority"
    );
    let mut substituted_sign = next_sign.clone();
    let AdapterEffect::Sign {
        request: SignRequest::Vote(vote),
        ..
    } = &mut substituted_sign
    else {
        unreachable!("fixture successor is a Vote Sign")
    };
    vote.subject = subject(0xD8);
    let substituted_sign_body = RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
        RecoveredLifecycleNextVoteBodyLookupV1::for_test(&prepare, expected_manifest_hash)
            .expect("project exact reducer body lookup"),
        validated.clone(),
        body_store_identity.clone(),
    )
    .expect("exact body mints opaque test authority");
    assert!(
        preview
            .project_broadcast_and_substituted_sign_for_test(
                &substituted_sign,
                substituted_sign_body,
            )
            .is_err(),
        "a substituted next Sign cannot consume exact body authority"
    );

    let dispatch_key = preview.dispatch_key();
    let exact_body = RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
        exact_body_lookup,
        validated.clone(),
        body_store_identity.clone(),
    )
    .expect("exact body mints opaque test authority");
    assert!(exact_body.exactly_matches_for_test(&validated, &body_store_identity));
    let combined = preview
        .project_broadcast_and_sign_authority(exact_body)
        .expect("seal exact recovered Broadcast and next Sign");
    let duplicate_body = RecoveredLifecycleNextVoteBodyAuthorityV1::for_test(
        RecoveredLifecycleNextVoteBodyLookupV1::for_test(&prepare, expected_manifest_hash)
            .expect("project exact reducer body lookup"),
        validated.clone(),
        body_store_identity,
    )
    .expect("exact body mints a separate opaque test authority");
    assert!(
        preview
            .project_broadcast_and_sign_authority(duplicate_body)
            .is_err(),
        "one preview cannot duplicate its combined successor authority"
    );
    assert!(combined.exactly_matches_for_test(
        dispatch_key,
        &broadcast,
        prepare_identity,
        &next_sign,
        &validated,
    ));
    let mut substituted_broadcast = broadcast.clone();
    let AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
        ..
    }) = &mut substituted_broadcast
    else {
        unreachable!("fixture Broadcast is the signed Proposal")
    };
    proposal.signature.push(0xD9);
    assert!(
        !combined.exactly_matches_for_test(
            dispatch_key,
            &substituted_broadcast,
            prepare_identity,
            &next_sign,
            &validated,
        ),
        "the combined authority retains the exact signed Broadcast"
    );
    assert!(
        !combined.exactly_matches_for_test(
            dispatch_key,
            &broadcast,
            proposal_identity,
            &next_sign,
            &validated,
        ),
        "a different authenticated WAL frame cannot own the next Sign"
    );
    assert!(
        !combined.exactly_matches_for_test(
            dispatch_key,
            &broadcast,
            prepare_identity,
            &substituted_sign,
            &validated,
        ),
        "the combined authority retains the exact next Sign"
    );
    assert!(
        !combined.exactly_matches_for_test(
            dispatch_key,
            &broadcast,
            prepare_identity,
            &next_sign,
            &foreign_body,
        ),
        "the combined authority retains the exact validated body"
    );

    let cold_adapter = RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
        broadcast,
        next_sign,
    )
    .expect("the exact durable pair mints one cold-adapter authority");
    drop(combined);
    drop(preview);
    let verified = VerifiedHeightContext::genesis(context, proofs)
        .expect("reverify the exact recovered Proposal context");
    let confirmed = ProductionLifecycleAdapterStartupV1::recovered(adapter, Vec::new())
        .advance_recovered_lifecycle_signed_broadcast_and_sign(&verified, cold_adapter)
        .expect("cold adapter replays the exact fsynced Broadcast-and-Sign pair");
    let ProductionLifecycleAdapterStartupStateV1::Recovered {
        adapter: confirmed,
        effects: confirmed_effects,
        leader_wire_launch_prepared: false,
        ..
    } = confirmed.state
    else {
        panic!("confirmed production startup remains in the recovered state")
    };
    assert!(confirmed_effects.is_empty());
    let Some(reducer::SignableMessage::Vote(awaiting)) = confirmed.reducer.awaiting_signature()
    else {
        panic!("confirmed adapter must await the exact next Vote Sign")
    };
    assert_eq!(
        confirmed
            .registry
            .unsigned_vote_to_wire(*awaiting)
            .expect("reconstruct confirmed next Vote"),
        prepare
    );
}

#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn production_recovered_proposal_sign_joins_exact_next_vote_body_store() {
    let directory = TempDir::new().expect("temporary production recovered-Sign directory");
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let local = context.leader(round.view);
    let local_index = usize::try_from(local).expect("fixture signer index fits usize");
    let header = BlockHeader::new(
        NonZeroU64::new(round.height).expect("fixture height is non-zero"),
        None,
        None,
        None,
        8_214,
        round.view,
    );
    let block_signature =
        SignatureOf::try_from_hash(keys[local_index].private_key(), header.hash())
            .expect("sign exact recovered-Sign body");
    let block = SignedBlock::presigned(
        BlockSignature::new(u64::from(local), block_signature),
        header,
        Vec::new(),
    );
    let canonical_wire = block
        .encode_wire()
        .expect("encode exact recovered-Sign body");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let outbound = encode_payload(&context, round, subject, &canonical_wire)
        .expect("encode exact recovered-Sign payload");
    let manifest = outbound.manifest().clone();
    let proposal = wire::Proposal {
        round,
        proposer: local,
        subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    let commitment = execution_commitment(0xD6);
    let prepare = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: commitment,
        signer: local,
        signature: Vec::new(),
    };
    let RecoveredAdapterStartup { adapter, effects } = write_and_reopen_authenticated_wal_startup(
        &directory,
        &context,
        &proofs,
        local,
        [0xD6; 32],
        vec![WalRecordV2::ProposalIntent(proposal.clone())],
    );
    let RecoveredAdapterStartup {
        adapter: cold_adapter,
        effects: cold_effects,
    } = write_and_reopen_authenticated_wal_startup_at_path(
        directory.path().join("cold-pair-preview-safety.wal"),
        &context,
        &proofs,
        local,
        [0xD6; 32],
        vec![
            WalRecordV2::ProposalIntent(proposal.clone()),
            WalRecordV2::PrepareIntent(prepare.clone()),
        ],
    );
    assert_eq!(cold_effects, effects);
    let [AdapterEffect::Sign { tag, request }] = effects.as_slice() else {
        panic!("recovered Proposal/Prepare FIFO must expose the Proposal Sign first")
    };
    assert_eq!(request, &SignRequest::Proposal(proposal.clone()));
    let tag = *tag;
    let request = request.clone();
    let prepare_identity = cold_adapter
        .authenticate_recovered_wal_frame(&cold_adapter.wal.recovered_records()[1])
        .expect("authenticate exact PrepareIntent frame")
        .0;
    let signature = Signature::new(
        keys[local_index].private_key(),
        &request.signature_preimage(),
    )
    .payload()
    .to_vec();
    let completion = || {
        super::super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
            1,
            tag,
            request.clone(),
            signature.clone(),
            Some(outbound.clone()),
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1::ControlProposal,
        )
    };

    let mut body_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        directory.path().join("exact-next-vote-body"),
        context.clone(),
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open exact next-Vote body store");
    let durable = body_store
        .store(manifest.clone(), canonical_wire)
        .expect("persist exact next-Vote body");
    let validated = body_store
        .validate(&durable, |_| Ok::<_, String>(commitment))
        .expect("persist exact next-Vote validation");
    let body_store_identity = body_store.instance_identity();

    let [
        AdapterEffect::Sign {
            tag: cold_tag,
            request: cold_request,
        },
    ] = cold_effects.as_slice()
    else {
        panic!("cold recovered Proposal/Prepare FIFO retains one Proposal Sign")
    };
    let SignRequest::Proposal(mut signed_proposal) = cold_request.clone() else {
        panic!("cold recovered control request is a Proposal")
    };
    signed_proposal.signature.clone_from(&signature);
    let cold_broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(signed_proposal),
    ));
    let mut substituted_cold_broadcast = cold_broadcast.clone();
    let AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::Proposal(substituted_proposal),
        ..
    }) = &mut substituted_cold_broadcast
    else {
        unreachable!("cold fixture Broadcast is a signed Proposal")
    };
    substituted_proposal.subject.payload_hash = Hash::new(b"substituted cold Proposal payload");
    assert!(
        RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            *cold_tag,
            cold_request.clone(),
            substituted_cold_broadcast,
        )
        .is_none(),
        "a substituted signed Broadcast cannot mint cold preview authority"
    );
    let cold_authority =
        RecoveredLifecycleSignedBroadcastColdPreviewAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            *cold_tag,
            cold_request.clone(),
            cold_broadcast.clone(),
        )
        .expect("WAL-authenticated Proposal Broadcast mints one cold preview authority");
    drop(cold_effects);
    let verified = VerifiedHeightContext::genesis(context.clone(), proofs.clone())
        .expect("reverify cold recovered Proposal context");
    let recovered_local_proposal =
        RecoveredLifecycleLocalProposalAttemptV1::for_test(tag, proposal.round, proposal.subject);
    let mut cold_preview =
        ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt(
            cold_adapter,
            Vec::new(),
            Some(recovered_local_proposal),
        )
        .prepare_recovered_lifecycle_signed_broadcast_and_sign(&verified, cold_authority)
        .expect("cold adapter previews exact Broadcast and next Sign");
    let cold_body = body_store
        .authenticate_recovered_lifecycle_next_vote_body(&mut cold_preview)
        .expect("exact revalidated body store authenticates the cold next Vote");
    assert!(cold_body.exactly_matches_for_test(&validated, &body_store_identity,));
    assert!(matches!(
        body_store.authenticate_recovered_lifecycle_next_vote_body(&mut cold_preview),
        Err(super::super::v2_body_store::V2BodyStoreError::RecoveredLifecycleNextVoteBodyMismatch)
    ));
    let cold_seal = cold_preview
        .seal_recovered_lifecycle_next_wal_vote(cold_body)
        .expect("cold preview seals its exact WAL and body-owned next Vote");
    let (cold_startup, sealed_broadcast, sealed_next_sign, sealed_output) = cold_seal
        .consume_for_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
        );
    let expected_next_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(prepare.clone()),
    };
    assert_eq!(sealed_broadcast, cold_broadcast);
    assert!(
        sealed_output
            .as_ref()
            .is_some_and(|output| output.matches_broadcast(&sealed_broadcast))
    );
    assert!(sealed_next_sign.exactly_matches(prepare_identity, &expected_next_sign, &validated,));
    let cold_adapter_authority =
        RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(
            super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignBroadcastProjectionPermitV1::for_test(),
            sealed_broadcast,
            expected_next_sign,
        )
        .expect("sealed cold pair retains the exact adapter replay relation");
    drop(sealed_next_sign);
    let cold_startup = cold_startup
        .advance_recovered_lifecycle_signed_broadcast_and_sign(&verified, cold_adapter_authority)
        .expect("sealed cold pair advances the retained original startup");
    let ProductionLifecycleAdapterStartupStateV1::Recovered {
        adapter: advanced_cold_adapter,
        effects: advanced_cold_effects,
        local_proposal_attempt: Some(recovered_local_proposal),
        leader_wire_launch_prepared: false,
        ..
    } = cold_startup.state
    else {
        panic!("advanced cold preview retains one recovered adapter startup")
    };
    assert!(advanced_cold_effects.is_empty());
    assert!(
        recovered_local_proposal.exactly_matches_directive(
            advanced_cold_adapter
                .local_proposal_directive()
                .expect("read the advanced cold Proposal directive"),
        ),
        "cold Broadcast-and-Sign replay must preserve its opaque local-attempt owner"
    );
    let Some(reducer::SignableMessage::Vote(advanced_vote)) =
        advanced_cold_adapter.reducer.awaiting_signature()
    else {
        panic!("advanced cold preview must await its exact next Vote")
    };
    assert_eq!(
        advanced_cold_adapter
            .registry
            .unsigned_vote_to_wire(*advanced_vote)
            .expect("reconstruct advanced cold next Vote"),
        prepare,
    );

    let now = Instant::now();
    let (mut runtime, startup_effects) = super::super::v2_runtime::SerializedV2Runtime::new(
        adapter,
        effects,
        now,
        Duration::from_secs(10),
        super::super::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap recovered adapter in the serialized runtime");
    let _startup_ownership = runtime
        .take_effect_ownership(startup_effects.len())
        .expect("transfer the recovered Sign's startup ownership");
    let output_guard = super::super::output_guard::ConsensusOutputGuard::isolated();
    let requester = context.roster[local_index].validator.clone();
    let (mut executor, body_store) =
        super::super::v2_effects::V2EffectExecutor::open_with_body_store(
            runtime,
            body_store,
            context.clone(),
            requester,
            Some(local),
            Arc::clone(&output_guard),
            super::super::v2_effects::EffectQueueConfig::default(),
        )
        .expect("open executor with exact recovered body catalogs");

    let (mut services, _) = super::super::v2_worker::tests::fixture();
    let service_io =
        super::super::v2_worker::tests::install_lifecycle_planner_io_for_validator_for_test(
            &mut services,
            context.clone(),
            tag,
            local,
            Arc::clone(&output_guard),
            body_store,
            body_store_identity.clone(),
            1,
        );
    super::super::v2_worker::tests::install_local_signer_for_test(
        &mut services,
        &keys[local_index],
    );
    let foreign_store = super::super::v2_body_store::V2BodyStore::open_with_policy(
        directory.path().join("foreign-next-vote-body"),
        context.clone(),
        super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open foreign next-Vote body store");
    let foreign_store_identity = foreign_store.instance_identity();
    assert!(!foreign_store_identity.same_instance(&body_store_identity));
    let (mut foreign_services, _) = super::super::v2_worker::tests::fixture();
    let foreign_service_io =
        super::super::v2_worker::tests::install_lifecycle_planner_io_for_validator_for_test(
            &mut foreign_services,
            context,
            tag,
            local,
            Arc::clone(&output_guard),
            foreign_store,
            foreign_store_identity,
            1,
        );

    let status_before = executor.status();
    let tag_before = executor.current_tag();
    let error = match foreign_services
        .prepare_recovered_lifecycle_sign_completion_with_body(&mut executor, completion())
    {
        Err(error) => error,
        Ok(_) => panic!("a foreign body-store service must fail before adapter preview"),
    };
    assert!(error.contains("foreign service owner"));
    let mut status_after = executor.status();
    status_after.captured_at = status_before.captured_at;
    assert_eq!(status_after, status_before);
    assert_eq!(executor.current_tag(), tag_before);
    assert!(!output_guard.restart_required());

    let (mut preview, body_authority) = services
        .prepare_recovered_lifecycle_sign_completion_with_body(&mut executor, completion())
        .expect("the exact production service authenticates the next-Vote body");
    assert_eq!(
        preview.shape(),
        RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
    );
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal)
    );
    assert!(body_authority.exactly_matches_for_test(&validated, &body_store_identity));
    let dispatch_key = preview.dispatch_key();
    let broadcast = preview.broadcast_effect().clone();
    assert!(matches!(
        &broadcast,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(signed),
            ..
        }) if signed.manifest == manifest && signed.signature == signature
    ));
    assert!(preview.next_sign_effect().is_none());
    let proposal_output = preview
        .project_proposal_exact_output_authority()
        .expect("seal the signed Proposal and exact recovered payload");
    assert!(
        preview.project_proposal_exact_output_authority().is_err(),
        "one adapter preview cannot duplicate Proposal output authority"
    );
    let proposal_output = match services
        .capture_recovered_lifecycle_proposal_exact_output(proposal_output)
        .expect("exact launched service reserves Proposal control and chunks")
    {
        super::super::v2_worker::RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(
            reservation,
        ) => reservation.abort_before_publication(),
        super::super::v2_worker::RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("empty exact-output corridor must retain the complete Proposal batch")
        }
    };
    let mut output = match services
        .capture_recovered_lifecycle_proposal_exact_output(proposal_output)
        .expect("typed abort returns the exact retry authority")
    {
        super::super::v2_worker::RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(
            reservation,
        ) => reservation,
        super::super::v2_worker::RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("retry against the unchanged empty corridor must remain reservable")
        }
    };
    let wal_permit = output
        .prepare_wal_append_permit()
        .expect("the armed Proposal output owns the initial WAL append");
    preview
        .append_recovered_lifecycle_proposal_prepare_wal(wal_permit)
        .expect("fsync the preflighted PrepareIntent before child publication");
    assert!(
        output.prepare_wal_append_permit().is_none(),
        "a successful WAL append irreversibly closes the retry permit"
    );
    assert_eq!(
        preview.shape(),
        RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign
    );
    assert_eq!(
        preview.settlement_family(),
        Some(RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign)
    );
    assert_eq!(preview.adapter.wal.recovered_records().len(), 2);
    assert_eq!(preview.adapter.pending_persistence_id, Some(2));
    let appended_prepare_identity = preview
        .adapter
        .authenticate_recovered_wal_frame(&preview.adapter.wal.recovered_records()[1])
        .expect("authenticate the just-fsynced PrepareIntent")
        .0;
    assert_eq!(appended_prepare_identity, prepare_identity);
    let next_sign = preview
        .next_sign_effect()
        .expect("the fsynced PrepareIntent retains its exact Prepare Sign")
        .clone();
    assert!(matches!(
        &next_sign,
        AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        } if vote == &prepare
    ));
    let combined = preview
        .project_broadcast_and_sign_authority(body_authority)
        .expect("seal the exact production-authenticated successor pair");
    assert!(combined.exactly_matches_for_test(
        dispatch_key,
        &broadcast,
        prepare_identity,
        &next_sign,
        &validated,
    ));
    drop(combined);
    preview.commit_after_durable_broadcast_and_sign();
    output.commit_after_publication();

    foreign_service_io.detach(&mut foreign_services);
    service_io.detach(&mut services);
}
