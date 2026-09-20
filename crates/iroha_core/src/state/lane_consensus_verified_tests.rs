// Native State/Kura authentication and publication-race controls.

// The economic fixture retains its large State allocation across nested block setup.
// Other fixtures keep their existing by-value owner through the default type.
struct LaneContextVerifiedFixture<StateOwner = State> {
    state: StateOwner,
    validators: Vec<KeyPair>,
    binding: crate::torii_proxy::QueuePlanAdmissionBindingV1,
    block: SignedBlock,
    opening: iroha_data_model::block::consensus_v2::HeightContext,
    witness: ExecWitness,
}

fn lane_context_verified_fixture() -> Box<LaneContextVerifiedFixture> {
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        )),
        &validators,
        parent.header().height().get(),
        0x4B,
    );
    let (block, opening, witness) =
        publish_lane_context_verified_fixture(&state, &parent, &certificate);
    Box::new(LaneContextVerifiedFixture {
        state,
        validators,
        binding,
        block,
        opening,
        witness,
    })
}

// State construction and its publication overlay need separate stack frames;
// callers retain the completed fixture on the heap, including foreign owners.
#[inline(never)]
fn publish_lane_context_verified_fixture(
    state: &State,
    parent: &SignedBlock,
    certificate: &[u8],
) -> (
    SignedBlock,
    iroha_data_model::block::consensus_v2::HeightContext,
    ExecWitness,
) {
    let block = lane_context_admission_carrier_for_test(parent, certificate);
    let opening = lane_opening_context_for_state_test(state);
    let predecessor_runtime = state.canonical_runtime.view().get().clone();
    let mut overlay = state
        .block_with_queue_plan_admissions(block.header(), &[certificate.to_vec()])
        .unwrap();
    overlay
        .finalize_lane_consensus_contexts(&block, Some(&opening))
        .unwrap();
    let mut witness = ExecWitness::default();
    overlay
        .capture_lane_consensus_contexts(&mut witness)
        .unwrap();
    overlay
        .stage_autoscale_sample_record_for_count(&block, 0)
        .unwrap();
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
    overlay.commit().unwrap();
    assert_eq!(
        state.canonical_runtime.predecessor_view().get().as_ref(),
        Some(&predecessor_runtime),
        "snapshot fixture must retain the actual pre-carrier runtime"
    );
    state.kura.store_block(Arc::new(block.clone())).unwrap();
    (block, opening, witness)
}

// Real test-key finality authenticates these exact fixture witness bytes. This
// exercises native finality and State association, not the complete executor.
fn stage_lane_context_fixture_finality(
    state: &State,
    block: &SignedBlock,
    opening: iroha_data_model::block::consensus_v2::HeightContext,
    mut witness: ExecWitness,
) -> (V2FinalityArtifact, crate::kura::KuraV2CommitReceipt) {
    let mut parent = None;
    for height in 1..block.header().height().get() {
        let artifact = state
            .kura
            .v2_finality_artifact(height)
            .unwrap()
            .unwrap_or_else(|| {
                let block = state
                    .kura
                    .get_block(NonZeroUsize::new(height as usize).unwrap())
                    .unwrap();
                let artifact = merge_carrier_finality_artifact_with_network(
                    &block,
                    parent.as_ref(),
                    state.network_id,
                );
                let _receipt = state.kura.store_v2_finality_artifact(&artifact).unwrap();
                artifact
            });
        parent = Some(artifact);
    }
    let height = block.header().height().get();
    let fee =
        iroha_data_model::validation_fee::ValidationFeePolicySnapshotCommitmentV1::from_registry(
            height, None,
        );
    let casting = iroha_data_model::parliament_casting::ParliamentTimedOvnCastingSnapshotCommitmentV1::from_ordered_bindings(height, &[]).unwrap();
    witness.writes.extend([
        iroha_data_model::block::consensus::ExecKv {
            key: iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_WITNESS_KEY_V1.to_vec(),
            value: norito::to_bytes(&fee).unwrap(),
        },
        iroha_data_model::block::consensus::ExecKv {
            key: iroha_data_model::parliament_casting::PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1
                .to_vec(),
            value: norito::to_bytes(&casting).unwrap(),
        },
    ]);
    let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::empty(
        block.encode_wire().unwrap().len() as u64,
        block.executed_block_wire_hash().unwrap(),
    );
    let commitment =
        crate::sumeragi::exec::execution_commitment_from_witness_for_tests(&witness, &manifest)
            .unwrap();
    let mut artifact =
        merge_carrier_finality_artifact_with_network(block, parent.as_ref(), state.network_id);
    artifact.height_context = opening;
    artifact.commit_qc.round.context_id = artifact.context_id();
    artifact.commit_qc.proposal_round.context_id = artifact.context_id();
    artifact.commit_qc.execution_commitment = commitment;
    let mut keys = (0xD3_u8..=0xD6)
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    assert_eq!(
        artifact
            .height_context
            .roster
            .iter()
            .map(|entry| entry.validator.public_key())
            .collect::<Vec<_>>(),
        keys.iter().map(KeyPair::public_key).collect::<Vec<_>>()
    );
    let preimage = artifact
        .commit_qc
        .signer_preimage(&artifact.height_context, 0)
        .unwrap();
    let signatures = keys
        .iter()
        .take(3)
        .map(|key| {
            Signature::try_new(key.private_key(), &preimage)
                .unwrap()
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    artifact.commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap();
    artifact.verify().unwrap();
    state
        .kura
        .stage_kagemusha_finality_sidecar(height, block.hash(), &witness, commitment, &[])
        .unwrap();
    let receipt = state.kura.store_v2_finality_artifact(&artifact).unwrap();
    (artifact, receipt)
}

state_test! { sync lane_consensus_verified_reader_requires_published_native_finality_and_exact_full_set
    let fixture = lane_context_verified_fixture();
    let state = &fixture.state;
    assert!(state.verified_lane_consensus_contexts().unwrap().is_none());
    let (artifact, receipt) = stage_lane_context_fixture_finality(state, &fixture.block, fixture.opening, fixture.witness);
    assert!(state.verified_lane_consensus_contexts().unwrap().is_none(), "a staged proof is not final publication");
    state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
    let observed: VerifiedLaneContexts = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(observed.is_current(state));
    assert_eq!(observed.carrier_height(), artifact.height);
    assert_eq!(observed.contexts().len(), 1);
    let verified = &observed.contexts()[0];
    let frozen = verified.frozen();
    assert_eq!(frozen.admitted_binding_hash, fixture.binding.canonical_hash());
    assert_ne!(frozen.committee, artifact.height_context.roster.iter().map(|entry| entry.validator.clone()).collect::<Vec<_>>(), "post-state lane authority is distinct from the global certifier");
    let core = verified.reducer_context();
    assert_eq!(core.height(), frozen.next_lane_height);
    assert_eq!(verified.instance_id().0.as_ref(), core.id().as_bytes());
    assert!(core.parent_commit().is_none());
    let anchor = core.finalized_state_anchor().unwrap();
    assert_eq!(anchor.height, artifact.height);
    assert_eq!(anchor.predecessor_height, 0);
    assert_eq!(anchor.predecessor_subject, None);
    let seed = Hash::new_from_chunks(&[
        b"iroha:lane-consensus:leader-seed:v1\0", &frozen.leader_seed,
        &frozen.lane_id.as_u32().to_be_bytes(), &frozen.dataspace_id.as_u64().to_be_bytes(),
        frozen.lane_incarnation.as_ref(), &frozen.next_lane_height.to_be_bytes(),
    ]);
    let offset = seed.as_ref().iter().fold(0usize, |n, byte| (n * 256 + usize::from(*byte)) % frozen.committee.len());
    assert_eq!(core.leader(0), core.roster()[offset].id());
    seed_autoscale_sample_history_for_snapshot_test(state);
    let restored = deserialize_state_snapshot_value_with_kura(
        norito::json::to_value(state).unwrap(), Arc::clone(&state.kura),
    ).unwrap();
    assert_eq!(restored.verified_lane_consensus_contexts().unwrap().unwrap().contexts()[0].instance_id(), verified.instance_id());
    let exact = state.lane_consensus_contexts.view().get().clone();
    for change in 0..3 {
        let mut cell = state.lane_consensus_contexts.block();
        *cell.get_mut() = exact.clone();
        match change {
            0 => cell.get_mut().contexts.clear(),
            1 => cell.get_mut().contexts[0].admitted_binding_hash = Hash::new(b"different pending head"),
            2 => cell.get_mut().contexts[0].leader_seed[0] ^= 1,
            _ => unreachable!(),
        }
        cell.commit();
        assert!(state.verified_lane_consensus_contexts().is_err(), "raw State mutation is not finality");
    }
}

state_test! { sync lane_consensus_verified_reader_rejects_signed_set_with_false_opening_policy
    for change in 0..3 {
        let mut fixture = lane_context_verified_fixture();
        let mut contexts = fixture.state.lane_consensus_contexts.view().get().clone();
        let frozen = &mut contexts.contexts[0];
        match change {
            0 => frozen.opening_global_context_id.0 = HashOf::from_untyped_unchecked(Hash::new(b"foreign opening context")),
            1 => frozen.execution_policy_hash = Hash::new(b"policy not used by opening carrier"),
            2 => frozen.leader_seed[0] ^= 1,
            _ => unreachable!(),
        }
        let commitment = LaneConsensusContextsCommitmentV1::from_contexts(
            fixture.state.network_id, fixture.block.header().height().get(), &contexts,
        ).unwrap();
        fixture.witness.writes[0].value = norito::to_bytes(&commitment).unwrap();
        let mut cell = fixture.state.lane_consensus_contexts.block();
        *cell.get_mut() = contexts;
        cell.commit();
        let (artifact, receipt) = stage_lane_context_fixture_finality(&fixture.state, &fixture.block, fixture.opening, fixture.witness);
        fixture.state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
        let error = fixture.state.verified_lane_consensus_contexts().unwrap_err();
        assert!(error.contains("canonical opening authority"), "{error}");
    }
}

state_test! { sync lane_consensus_verified_reader_does_not_infer_genesis_authority
    assert!(blank_test_state().verified_lane_consensus_contexts().unwrap().is_none());
}

state_test! { sync lane_consensus_verified_reader_keeps_opening_identity_and_authenticates_empty_closure
    let fixture = lane_context_verified_fixture();
    let state = &fixture.state;
    let (opening_artifact, receipt) = stage_lane_context_fixture_finality(state, &fixture.block, fixture.opening, fixture.witness);
    state.kura.promote_kagemusha_finality_sidecar(&opening_artifact, &receipt).unwrap();
    let before = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let initial_id = before.contexts()[0].instance_id();
    let mut parent = fixture.block;
    let mut previous_artifact = opening_artifact;
    for close in [false, true] {
        let block = empty_global_block_after(Some(&parent));
        let context = crate::sumeragi::v2_context::build_successor_height_context(
            &previous_artifact, previous_artifact.height_context.nexus_amx_context_hash, None,
        ).unwrap();
        let mut overlay = state.block(block.header());
        if close {
            assert!(State::resolve_queue_plan_pending_obligation_in_storage(
                &mut overlay.world.smart_contract_state, fixture.binding.network_id_digest,
                fixture.binding.entrypoint_hash,
            ).unwrap());
        }
        overlay.finalize_lane_consensus_contexts(&block, Some(&context)).unwrap();
        let mut witness = ExecWitness::default();
        overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
        overlay.block_hashes.push(block.hash());
        insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
        overlay.commit().unwrap();
        state.kura.store_block(Arc::new(block.clone())).unwrap();
        assert!(!before.is_current(state));
        assert!(state.verified_lane_consensus_contexts().unwrap().is_none(), "even empty State needs current finality");
        let (artifact, receipt) = stage_lane_context_fixture_finality(state, &block, context, witness);
        assert!(state.verified_lane_consensus_contexts().unwrap().is_none(), "even empty closure waits for final proof publication");
        state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
        let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
        assert!(current.is_current(state));
        assert_eq!(current.carrier_height(), block.header().height().get());
        if close {
            assert!(current.contexts().is_empty());
        } else {
            assert_eq!(current.contexts()[0].instance_id(), initial_id);
            assert_eq!(current.contexts()[0].frozen().opening_global_height, before.contexts()[0].frozen().opening_global_height);
            let opening_path = state.kura.v2_finality_artifact_path_for_testing(
                before.contexts()[0].frozen().opening_global_height,
            );
            let saved = std::fs::read(&opening_path).unwrap();
            std::fs::remove_file(&opening_path).unwrap();
            let missing = state.verified_lane_consensus_contexts();
            std::fs::write(&opening_path, saved).unwrap();
            assert!(missing.unwrap_err().contains("required historical lane opening finality"),
                "missing old custody cannot become an indefinite current-publication wait");
            assert_eq!(state.verified_lane_consensus_contexts().unwrap().unwrap().contexts()[0].instance_id(), initial_id);
        }
        previous_artifact = artifact;
        parent = block;
    }
}

state_test! { sync lane_consensus_verified_reader_releases_state_before_io_and_discards_changed_publication
    let fixture = lane_context_verified_fixture();
    let (artifact, receipt) = stage_lane_context_fixture_finality(&fixture.state, &fixture.block, fixture.opening, fixture.witness);
    fixture.state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
    let state = Arc::new(fixture.state);
    let observed = Arc::clone(&state);
    let successor = empty_global_block_after(Some(&fixture.block));
    let result = lane_consensus_verified::io_observer::observe(move || {
        let writer = Arc::clone(&observed);
        let header = successor.header();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let task = std::thread::spawn(move || {
            let _lease = writer.consensus_publication_lease();
            writer.append_committed_block_header_for_tests(header);
            // Acquiring a World writer also detects a retained StateView.
            let world = writer.world.block();
            world.commit();
            done_tx.send(()).unwrap();
        });
        done_rx.recv_timeout(Duration::from_secs(5)).expect("State publication must not wait for reader guards");
        task.join().unwrap();
    }, || state.verified_lane_consensus_contexts());
    assert!(result.unwrap().is_none(), "an exact old proof cannot survive a changed State publication");
}
