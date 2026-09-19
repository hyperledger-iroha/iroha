// Publish the pending retry through its actual ordinary admission carrier before
// taking a cold snapshot of an unfinished earlier Native publication.
fn enqueue_native_retry_with_canonical_admission(
    adapter: V2LaneWorkAdapter,
    keys: &[KeyPair],
    queue: &Arc<Queue>,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> V2LaneWorkAdapter {
    use crate::sumeragi::exec::{
        LaneFinalityManifestV1, NativeAmxApplicationManifestV1,
        execution_commitment_from_validated_block,
    };

    let index_directory = adapter
        .kura
        .store_root()
        .join("native_amx_publication_index");
    let retained_index = std::fs::read_dir(&index_directory)
        .expect("earlier Native publication retains its durable recovery owner")
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(retained_index.len(), 1);

    let key = KeyPair::try_from_seed(vec![0xA0; 32], Algorithm::Ed25519).unwrap();
    let authority = AccountId::new(key.public_key().clone());
    if adapter
        .state
        .world_view()
        .accounts()
        .get(&authority)
        .is_none()
    {
        let mut world = adapter.state.world.block();
        world.accounts.insert(
            authority.clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.commit();
    }
    let transaction = TransactionBuilder::new(
        adapter.context.network_id,
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "native predecessor retry".to_owned())])
    .with_admission_intent(
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    )
    .sign(key.private_key());
    let accepted =
        crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(transaction));
    let routing_plan = queue
        .route_plan_with_state(&accepted, &adapter.state)
        .unwrap();
    assert_eq!(
        routing_plan.coordinator_route(),
        RoutingDecision::new(lane_id, dataspace_id)
    );
    let admission_context = queue
        .plan_admission_context_with_state(&adapter.state, &routing_plan)
        .unwrap();
    let binding = crate::torii_proxy::new_queue_plan_admission_binding(
        adapter.state.network_id_ref(),
        accepted.entrypoint(),
        &routing_plan,
        admission_context,
        queue.queue_plan_admission_timestamp_ms_for(&accepted),
    )
    .unwrap();
    let certificate =
        queue_plan_materialized_certificate_for_binding(accepted.entrypoint(), &binding, keys);
    let _claim = queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            accepted,
            &adapter.state,
            routing_plan,
            &binding,
        )
        .expect("retain the exact durable queued owner before global admission");

    let parent_finality = adapter
        .kura
        .v2_finality_artifact(adapter.context.height - 1)
        .expect("read the exact committed parent finality")
        .expect("the ordinary admission successor has durable parent finality");
    let nexus_context_hash =
        super::super::v2_recovery::committed_nexus_amx_context_hash(adapter.state.as_ref())
            .unwrap();
    let mut context = crate::sumeragi::v2_context::build_successor_height_context_from_state(
        &parent_finality,
        &adapter.state.view(),
        nexus_context_hash,
    )
    .expect("derive admission authority from the actual committed parent and World");
    assert_eq!(context.height, adapter.context.height);
    assert_eq!(context.roster.len(), 4);
    assert_eq!(keys.len(), context.roster.len());
    for (validator, key) in context.roster.iter().zip(keys) {
        assert_eq!(validator.validator.public_key(), key.public_key());
    }
    let parent = adapter
        .kura
        .get_block(NonZeroUsize::new(adapter.state.committed_height()).unwrap())
        .unwrap();
    let now_ms = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let header = BlockHeader::new(
        NonZeroU64::new(context.height).unwrap(),
        Some(parent.hash()),
        None,
        now_ms
            .max(binding.enqueue_timestamp_ms)
            .max(parent.header().creation_time_ms + 1),
        0,
    );
    let leader = usize::try_from(context.leader(0)).unwrap();
    let signature = SignatureOf::try_from_hash(keys[leader].private_key(), header.hash()).unwrap();
    let mut block = SignedBlock::presigned(
        BlockSignature::new(u64::try_from(leader).unwrap(), signature),
        header,
        Vec::new(),
    );
    block.set_execution_context(Some(
        iroha_data_model::block::BlockExecutionContextBundle::new(Vec::new())
            .with_queue_plan_admissions(vec![certificate.clone()]),
    ));
    block
        .replace_signatures(BTreeSet::from([BlockSignature::new(
            u64::try_from(leader).unwrap(),
            SignatureOf::try_from_hash(keys[leader].private_key(), block.hash()).unwrap(),
        )]))
        .unwrap();
    // The consumed StateBlock's box must also leave scope before the adapter
    // moves into its successor construction.
    let (artifact, receipt) = {
        let mut overlay = Box::new(
            adapter
                .state
                .block_with_queue_plan_admissions(block.header(), &[certificate])
                .expect("stage the exact authenticated QueuePlan admission"),
        );
        ValidBlock::execute_block_outputs_and_capture_for_test(
            &mut block,
            &mut overlay,
            None,
            &mut context,
        )
        .expect("execute the actual ordinary outputs and capture their original witness");
        let witness = overlay
            .take_exec_witness()
            .expect("actual admission execution witness");
        let casting = overlay
            .take_parliament_timed_ovn_casting_bindings()
            .unwrap();
        let native =
            NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(&block, None)
                .unwrap();
        assert_eq!(
            native.count(),
            0,
            "admission does not reapply the earlier Native carrier"
        );
        let lanes = LaneFinalityManifestV1::from_result_bearing_block(&block).unwrap();
        let commitment =
            execution_commitment_from_validated_block(&witness, &native, &lanes, &block)
                .expect("the genuine quorum binds the complete admission execution");
        let artifact = signed_finality_artifact(
            &context,
            keys,
            &block,
            commitment,
            vec![0, 1, 2],
            [
                "encode admission proposal",
                "derive admission vote preimage",
                "admission signer",
                "sign actual admission execution",
                "aggregate three admission votes",
                "derive admission validator PoP",
                "verify actual admission finality",
            ],
        );
        let verified = crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone()).unwrap();
        let committed = ValidBlock::new_unverified_for_tests(block)
            .commit_with_verified_v2_artifact(verified, commitment)
            .unpack(|_| {})
            .expect("bind genuine finality to this exact ordinary execution");
        adapter
            .kura
            .stage_kagemusha_finality_sidecar(
                artifact.height,
                artifact.block_hash,
                &witness,
                commitment,
                &casting,
            )
            .unwrap();
        adapter.kura.store_block(committed.clone()).unwrap();
        let receipt = adapter.kura.store_v2_finality_artifact(&artifact).unwrap();
        assert_eq!(receipt.artifact_hash(), HashOf::new(&artifact));
        overlay
            .authorize_execution_output_publication(&committed, &witness)
            .unwrap();
        overlay
            .apply_without_execution_with_verified_v2_finality(&committed)
            .unwrap();
        overlay
            .commit()
            .expect("publish the exact admission and complete lane context set");
        (artifact, receipt)
    };
    adapter
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    let checkpoint =
        crate::snapshot::canonical_state_snapshot_hash(adapter.state.as_ref()).unwrap();
    adapter
        .kura
        .store_wsv_checkpoint(artifact.height, artifact.block_hash, checkpoint)
        .unwrap();
    adapter
        .kura
        .store_commit_manifest(
            crate::kura::CommitManifest::new(
                artifact.height,
                artifact.block_hash,
                None,
                None,
                checkpoint,
                None,
            )
            .with_authenticated_v2_commit_authority(&artifact),
        )
        .unwrap();

    assert_eq!(
        adapter
            .state
            .queue_plan_pending_binding_for_entrypoint(binding.entrypoint_hash.clone(),)
            .unwrap(),
        Some(binding.clone())
    );
    let verified_contexts = adapter
        .state
        .verified_lane_consensus_contexts()
        .unwrap()
        .expect("actual admission finality authenticates its newly open lane instances");
    assert_eq!(
        verified_contexts.contexts().len(),
        binding.admission_context.route_incarnations.len()
    );
    for route in &binding.admission_context.route_incarnations {
        let frozen = verified_contexts
            .contexts()
            .iter()
            .find(|lane| lane.frozen().lane_id == route.leg.route.lane_id)
            .expect("every pending route has its exact authenticated open instance")
            .frozen();
        assert_eq!(frozen.dataspace_id, route.leg.route.dataspace_id);
        assert_eq!(frozen.admitted_binding_hash, binding.canonical_hash());
        assert_eq!(frozen.opening_global_height, artifact.height);
    }
    let nexus_context_hash =
        super::super::v2_recovery::committed_nexus_amx_context_hash(adapter.state.as_ref())
            .unwrap();
    let successor = crate::sumeragi::v2_context::build_successor_height_context_from_state(
        &artifact,
        &adapter.state.view(),
        nexus_context_hash,
    )
    .unwrap();
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let mut reopened = restart
        .reopen_isolated(successor, true)
        .expect("reopen the same local adapter after exact ordinary admission finality");
    reopened
        .install_lane_drain_queue(Arc::clone(queue))
        .unwrap();
    let remaining_index = std::fs::read_dir(&index_directory)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        remaining_index, retained_index,
        "ordinary admission must preserve the earlier unfinished Native publication owner"
    );
    reopened
}
