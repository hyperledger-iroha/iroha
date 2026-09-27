/// Build State/Kura evidence for one committed ordinary block with a genuine four-validator
/// Commit QC and DA layout.
///
/// The block carries one signed `Log` transaction and one exact Network execution output. The
/// app uses its own network identity, distinct from the default test app, so tests can also
/// exercise cross-state mismatches.
pub(crate) fn app_with_finalized_block_for_test(
    persist_finality: bool,
) -> (SharedAppState, V2FinalityArtifact) {
    const HEIGHT: u64 = 1;
    let keypair = checked_torii_test_ed25519_keypair(0x31, "derive finalized-block fixture key");
    let chain: ChainId = "chain"
        .parse()
        .expect("finalized-block fixture chain label");
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"Torii finalized-block fixture genesis"),
    ));
    let app = mk_app_state_for_tests_with_world_and_options_and_network_id(
        World::default(),
        None,
        None,
        None,
        None,
        chain,
        network_id,
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let tx = checked_torii_test_transaction(
        TransactionBuilder::new(
            *app.state.network_id_ref(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "Torii finalized-block fixture".to_owned(),
        )]),
        &keypair,
        "sign finalized-block fixture transaction",
    );
    let header = BlockHeader::new(
        std::num::NonZeroU64::new(HEIGHT).expect("nonzero height"),
        None,
        None,
        0,
        0,
    );
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    builder.push_transaction(tx);
    let mut block = builder.build_with_signature(0, keypair.private_key());
    let proposal = block.canonical_resultless_proposal();
    crate::test_utils::attach_fixture_execution_outputs(
        &mut block,
        vec![
            iroha_data_model::block::execution_output::ExecutionOutputV1::Network(
                iroha_data_model::block::execution_output::NetworkExecutionOutputV1 {
                    input_index: 0,
                    result: iroha_data_model::transaction::TransactionResult::new(Ok(vec![])),
                    completions: vec![],
                },
            ),
        ],
    );
    assert!(block.has_results());
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    assert_eq!(block.execution_outputs().len(), 1);
    block
        .validate_output_merkle_cache()
        .expect("finalized-block fixture retains its exact Network output");
    block
        .replace_signatures(
            [checked_torii_test_block_signature(
                0,
                &keypair,
                &block.header(),
                "sign finalized-block fixture with its final proposal commitment",
            )]
            .into_iter()
            .collect(),
        )
        .expect("signature binds the complete finalized-block fixture proposal");
    let block_hash = block.hash();
    let mut validator_keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("derive deterministic finality validator")
        })
        .collect::<Vec<_>>();
    validator_keys.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let roster = validator_keys
        .iter()
        .zip([1_u64; 4])
        .map(|(key, power)| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power,
        })
        .collect::<Vec<_>>();
    let kagemusha_mint_finality_authority =
        iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1 {
            version: iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
            network_id: *app.state.network_id_ref(),
            generation: 0,
            validators: roster
                .iter()
                .enumerate()
                .map(|(index, validator)| {
                    let seed =
                        0xA0_u8 + u8::try_from(index).expect("four-validator fixture index");
                    iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                        &[seed; 32],
                        0,
                        validator.validator.clone(),
                    )
                    .expect("derive paired-Pasta finality fixture keys")
                })
                .collect(),
        };
    let kagemusha_mint_finality_authorization =
        iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1::genesis(
            &kagemusha_mint_finality_authority,
            10,
        )
        .expect("canonical finality roster identity");
    let context = HeightContext {
        network_id: *app.state.network_id_ref(),
        protocol_version: PROTOCOL_VERSION,
        height: HEIGHT,
        epoch: 0,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Npos,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid finality roster"),
        roster,
        kagemusha_mint_finality_authorization,
        kagemusha_mint_finality_authority,
        nexus_amx_context_hash: Hash::new(b"Torii finalized-block exact-v2 finality context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: iroha_data_model::block::consensus_v2::recommended_data_availability_layout(),
        leader_seed: [0x42; 32],
    };
    context.validate().expect("valid finality height context");
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash,
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("hash exact finalized-block fixture proposal wire"),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height: HEIGHT,
        view: block.header().view_change_index(),
    };
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"Torii finalized-block exact-v2 parent state"),
            Hash::new(b"Torii finalized-block exact-v2 post state"),
            Hash::new(b"Torii finalized-block exact-v2 ordinary writes"),
            u64::try_from(block.encode_wire().expect("exact block wire").len())
                .expect("exact block wire length fits u64"),
            block
                .executed_block_wire_hash()
                .expect("hash exact finalized-block fixture block wire"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let preimage = commit_qc
        .signer_preimage(&context, 0)
        .expect("valid finality signer");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                validator_keys[usize::try_from(*index).expect("fixture signer index")]
                    .private_key(),
                &preimage,
            )
            .expect("sign exact Commit vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate exact Commit votes");
    let validator_set_pops = validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("derive finality validator PoP")
        })
        .collect();
    let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    artifact
        .validate_for_header(&block.header())
        .expect("finality fixture binds the exact block header");
    artifact
        .verify()
        .expect("finality fixture is cryptographically valid");
    let block_header = block.header();
    let stored_block_hash = store_block(&app, block);
    assert_eq!(stored_block_hash, artifact.block_hash);
    record_committed_block_hash_for_test(&app, block_header, stored_block_hash);
    if persist_finality {
        let receipt = app
            .kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist exact v2 finality artifact");
        assert_eq!(receipt.height(), artifact.height);
        assert_eq!(receipt.block_hash(), artifact.block_hash);
        assert_eq!(receipt.context_id(), artifact.context_id());
        assert_eq!(receipt.subject(), artifact.subject);
        assert_eq!(receipt.certificate(), artifact.commit_qc.as_ref());
        assert_eq!(receipt.artifact_hash(), HashOf::new(&artifact));
    }
    (app, artifact)
}
#[tokio::test]
async fn finalized_block_fixture_commits_one_ordinary_block_with_durable_finality() {
    let (app, artifact) = app_with_finalized_block_for_test(true);
    assert_ne!(
        *app.state.network_id_ref(),
        *mk_app_state_for_tests().state.network_id_ref(),
        "the fixture network must differ from the default test app"
    );
    let block = app
        .state
        .block_by_height(NonZeroUsize::new(1).expect("nonzero height"))
        .expect("committed fixture block");
    assert_eq!(block.hash(), artifact.block_hash);
    assert_eq!(block.execution_outputs().len(), 1);
    assert_eq!(
        artifact.height_context.network_id,
        *app.state.network_id_ref()
    );
    let (unpersisted, unpersisted_artifact) = app_with_finalized_block_for_test(false);
    assert_eq!(unpersisted_artifact.height, artifact.height);
    assert!(
        !unpersisted
            .kura
            .v2_finality_artifact_path_for_testing(1)
            .exists(),
        "without persist_finality the fixture stores no finality sidecar"
    );
}
