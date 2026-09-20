// Genuine four-validator finality for the native execution publication fixtures.

fn finalize_native_execution_for_test(
    state: &State,
    block: SignedBlock,
    overlay: &mut StateBlock<'_>,
    context: HeightContext,
) -> (crate::block::CommittedBlock, ExecWitness) {
    use crate::sumeragi::exec::{
        LaneFinalityManifestV1, NativeAmxApplicationManifestV1,
        execution_commitment_from_validated_block,
    };

    context.validate().expect("valid native carrier authority");
    assert_eq!(context.network_id, state.network_id);
    assert_eq!(context.height, block.header().height().get());
    assert_eq!(
        context.roster.len(),
        4,
        "exact four-validator carrier committee"
    );
    let candidates = (0xD3_u8..=0xD6)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("derive actual merge-carrier finality fixture key")
        })
        .collect::<Vec<_>>();
    let keys = context
        .roster
        .iter()
        .map(|validator| {
            assert_eq!(validator.power, 1, "fixture votes have equal power");
            candidates
                .iter()
                .find(|key| key.public_key() == validator.validator.public_key())
                .expect("carrier roster must match its actual fixture signing keys")
        })
        .collect::<Vec<_>>();
    let witness = overlay
        .take_exec_witness()
        .expect("native execution retains its actual complete captured witness");
    let casting = overlay
        .take_parliament_timed_ovn_casting_bindings()
        .expect("native execution retains its actual casting bindings");
    let native = NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &block,
        overlay.staged_merge_entry(),
    )
    .expect("native application manifest binds the actual staged merge entry");
    let lanes = LaneFinalityManifestV1::from_result_bearing_block(&block)
        .expect("native lane finality manifest binds the actual carrier");
    let execution_commitment =
        execution_commitment_from_validated_block(&witness, &native, &lanes, &block)
            .expect("carrier finality binds actual execution and its complete result wire");
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("canonical native carrier proposal"),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: block.header().view_change_index(),
    };
    let preimage = iroha_data_model::block::consensus_v2::Vote {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = keys
        .iter()
        .take(3)
        .map(|key| {
            Signature::try_new(key.private_key(), &preimage)
                .expect("sign actual native execution")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate exactly three native carrier finality votes");
    let certificate = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature,
    };
    let pops = keys
        .iter()
        .map(|key| bls_normal_pop_prove(key.private_key()).expect("actual carrier validator PoP"))
        .collect();
    let artifact = V2FinalityArtifact::new(context, subject, certificate, pops);
    let verified = crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone())
        .expect("native carrier finality passes genuine cryptographic verification");
    let committed = crate::block::ValidBlock::new_unverified_for_tests(block)
        .commit_with_verified_v2_artifact(verified, execution_commitment)
        .unpack(|_| {})
        .expect("verified carrier finality binds this exact execution");
    state
        .kura
        .stage_kagemusha_finality_sidecar(
            artifact.height,
            artifact.block_hash,
            &witness,
            execution_commitment,
            &casting,
        )
        .expect("stage actual native witness before State publication");
    state
        .kura
        .store_block(committed.clone())
        .expect("durably persist complete native result wire");
    let receipt = state
        .kura
        .store_v2_finality_artifact(&artifact)
        .expect("durably persist its exact genuine finality");
    assert_eq!(receipt.artifact_hash(), HashOf::new(&artifact));
    assert_eq!(receipt.block_hash(), committed.as_ref().hash());
    // Witness promotion remains after the caller's successful State commit.
    (committed, witness)
}

fn promote_native_execution_finality_for_test(state: &State, block: &crate::block::CommittedBlock) {
    let artifact = block
        .verified_v2_finality_artifact()
        .expect("native publication retains verified finality authority");
    let receipt = state
        .kura
        .store_v2_finality_artifact(artifact)
        .expect("recover the exact durable native finality receipt");
    state
        .kura
        .promote_kagemusha_finality_sidecar(artifact, &receipt)
        .expect("publish actual native witness only after State publication");
}
