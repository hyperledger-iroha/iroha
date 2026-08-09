#[test]
fn serialized_runtime_rebinds_busy_deferred_body_completion_before_service() {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS validator key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let roster = keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let context = wire::HeightContext {
        chain_id: "serialized-body-rebind-test".into(),
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"serialized rebind nexus context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1_048_576,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1_048_576,
            max_chunk_count: 2,
        },
        leader_seed: [0x44; 32],
    };
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified =
        VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified context");
    let directory = TempDir::new().expect("temporary runtime directory");
    let (mut adapter, startup) = SumeragiV2Adapter::open(
        directory.path().join("serialized-rebind-safety.wal"),
        verified,
        None,
        Generation::new(1),
        [0x55; 32],
        AdapterFingerprints {
            node: Hash::new(b"serialized rebind node"),
            build: Hash::new(b"serialized rebind build"),
            config: Hash::new(b"serialized rebind config"),
        },
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open observing adapter");
    assert!(startup.is_empty());

    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        3_000,
        0,
    );
    let block_signature = SignatureOf::try_from_hash(keys[0].private_key(), header.hash())
        .expect("canonical body signature");
    let block = SignedBlock::presigned(BlockSignature::new(0, block_signature), header, Vec::new());
    let body = block.encode_wire().expect("canonical SignedBlockWire");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&body),
    };
    let manifest = canonical_payload_manifest(&context, round, subject, &body);
    let execution_commitment = fixture_execution_commitment();
    let prepare_preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let prepare_shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &prepare_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let prepare_refs = prepare_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
            .expect("aggregate PrepareQC"),
    };
    let signed_timeout = |signer: wire::ValidatorIndex| {
        let mut vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(prepare.clone()),
            signer,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(signer).expect("small signer")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(vote))
    };

    for signer in 0_u32..2 {
        let authenticated = adapter
            .authenticate(signed_timeout(signer))
            .expect("authenticate timeout vote");
        adapter
            .receive_authenticated(authenticated)
            .expect("admit timeout share before quorum");
    }
    let original_tag = adapter.current_tag();
    adapter
        .defer_body_available_for_test(original_tag, &manifest)
        .expect("stage Busy-deferred body completion");
    let authenticated = adapter
        .authenticate(signed_timeout(2))
        .expect("authenticate quorum timeout vote");
    let final_effects = adapter
        .receive_authenticated(authenticated)
        .expect("form and install TC before draining the old completion")
        .into_effects();
    let rebound_tag = final_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::EnterView {
                tag,
                protected_body: Some(protected),
                ..
            } if *protected == (round, subject) => Some(*tag),
            _ => None,
        })
        .expect("effective-lock EnterView effect");

    let started = Instant::now();
    let (runtime, startup_effects) = SerializedV2Runtime::new(
        adapter,
        final_effects.clone(),
        started,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("serialized production runtime");
    assert_eq!(startup_effects, final_effects);
    let mut executor = V2EffectExecutor::with_runtime(
        runtime,
        BTreeMap::new(),
        context,
        PeerId::new(keys[3].public_key().clone()),
        None,
        EffectQueueConfig::default(),
    )
    .expect("serialized production executor");
    executor.ready_body_bytes = u64::try_from(body.len()).expect("body length");
    executor.ready_bodies.insert(
        (round, subject),
        ReadyBody {
            manifest: manifest.clone(),
            bytes: body.into(),
        },
    );
    executor.body_pipeline_owners.insert(
        (round, subject),
        BodyPipelineOwner {
            tag: original_tag,
            manifest_hash: Some(HashOf::new(&manifest)),
        },
    );
    let mut services = FakeServices::default();
    executor
        .consume_effects(final_effects, &mut services)
        .expect("executor rebinds the deferred completion before later service");
    assert!(services.fetch_tasks.is_empty());
    assert_eq!(
        executor.body_pipeline_owners[&(round, subject)].tag,
        rebound_tag
    );

    executor
        .arm_live_clocks(started)
        .expect("arm clocks after startup effects");
    assert!(matches!(
        executor
            .step(started + Duration::from_secs(2), &mut services)
            .expect("periodic service drains the rebound completion"),
        EffectExecutorStep::Advanced { .. }
    ));
    assert_eq!(services.store_tasks.len(), 1);
    assert_eq!(services.store_tasks[0].tag(), rebound_tag);
    assert_eq!(services.store_tasks[0].manifest(), &manifest);
    assert!(!executor.status().fail_closed);
}
