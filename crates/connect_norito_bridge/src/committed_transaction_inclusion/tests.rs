use super::*;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    block::{
        BlockHeader, SignedBlock,
        builder::BlockBuilder,
        consensus_v2::*,
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
        output_budget::ExecutionOutputLimits,
    },
    bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeCommitment, BridgeFinalityProof},
    query::{QueryOutput, QueryOutputBatchBoxTuple},
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use std::{num::NonZeroU64, time::Duration};

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0xa5; 32]),
    ))
}

fn selected_block() -> (SignedBlock, CommittedTransaction) {
    let key = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).unwrap();
    let authority = AccountId::new(key.public_key().clone());
    let mut transaction = TransactionBuilder::new(
        network(),
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(Duration::from_millis(1));
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 11, 0);
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(transaction.try_sign(key.private_key()).unwrap());
    let mut block = builder
        .try_build_with_signature(0, key.private_key())
        .unwrap();
    let output = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: 0,
        result: Ok(iroha_data_model::transaction::DataTriggerSequence::default()).into(),
        completions: Vec::new(),
    });
    block
        .set_execution_outputs(
            vec![output.clone()],
            1,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &ExecutionOutputLimits {
                max_outputs: 16,
                max_output_bytes: 64 * 1024,
                max_total_output_bytes: 256 * 1024,
                max_executed_wire_bytes: 1024 * 1024,
            },
        )
        .unwrap();
    let committed = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: block.network_entrypoint_at(0).unwrap().hash(),
        entrypoint_proof: block.network_input_proof(0).unwrap(),
        entrypoint: block.network_entrypoint_at(0).unwrap().clone(),
        output_hash: HashOf::new(&output),
        output_proof: block.output_proof(0).unwrap(),
        output,
    };
    assert!(committed.verify_inclusion_in_block(&block));
    (block, committed)
}

fn mint_authorization(
    roster: &[ValidatorPower],
) -> (
    iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1,
    iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1,
) {
    use iroha_data_model::isi::kagemusha_v1::{
        BeaconEpochBindingV1, KAGEMUSHA_CHAIN_VERSION_V1,
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1, KagemushaMintFinalityValidatorKeysV1,
    };
    const EQ: [&str; 4] = [
        "00000000ed302d991bf94c09fc98462200000000000000000000000000000040",
        "030000b067c50313fcac1144eee2fe0e0000000000000000000000000000001c",
        "63d232eb3b8af0b75cfcf55ade47f6ff4cdf4e47a7454cb8ed67a9ba6f56e788",
        "fc86bc8efbbcb878f49427618b6940409b9157e3d777a4c4c0514a8e0d92db18",
    ];
    const EP: [&str; 4] = [
        "0000000021eb468cdda89409fc98462200000000000000000000000000000040",
        "03000070de065fede0093144eee2fe0e0000000000000000000000000000001c",
        "5fce556feb6fee5a15560ddabae10224b026a5d0281af4c613955c39a8797837",
        "f79037a77e26a2c0794dc326d866c664616499c064073a8f8ebf3080297be5ab",
    ];
    assert_eq!(roster.len(), 4);
    let authority = KagemushaMintFinalityAuthorityGenerationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: network(),
        generation: 0,
        validators: roster
            .iter()
            .enumerate()
            .map(|(index, validator)| {
                let mut eq = [0; 32];
                let mut ep = [0; 32];
                hex::decode_to_slice(EQ[index], &mut eq).unwrap();
                hex::decode_to_slice(EP[index], &mut ep).unwrap();
                KagemushaMintFinalityValidatorKeysV1 {
                    validator: validator.validator.clone(),
                    eq_proof_public_key: eq,
                    ep_proof_public_key: ep,
                }
            })
            .collect(),
    };
    let authorization = KagemushaMintFinalityEpochAuthorizationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: network(),
        epoch: 0,
        first_height: 1,
        last_height: 10,
        authority_generation: authority.generation,
        authority_id: authority.authority_id().unwrap(),
        beacon: BeaconEpochBindingV1::Bootstrap,
        previous_authorization_id: [0; 32],
        transition_id: [0; 32],
        decision: KagemushaMintFinalityEpochDecisionV1::Genesis,
    };
    authorization
        .validate_against_authority(&authority)
        .unwrap();
    (authorization, authority)
}

fn finality_bundle(block: &SignedBlock) -> BridgeFinalityBundle {
    let mut keys = (0..4)
        .map(|_| KeyPair::try_random_with_algorithm(Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        iroha_model_base::peer::PeerId::new(left.public_key().clone()).cmp(
            &iroha_model_base::peer::PeerId::new(right.public_key().clone()),
        )
    });
    let roster = keys
        .iter()
        .map(|key| ValidatorPower {
            validator: iroha_model_base::peer::PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let proofs_of_possession = keys
        .iter()
        .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
        .collect::<Vec<_>>();
    let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
        mint_authorization(&roster);
    let header = block.header();
    let context = HeightContext {
        network_id: network(),
        protocol_version: PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        kagemusha_mint_finality_authorization,
        kagemusha_mint_finality_authority,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).unwrap(),
        roster,
        nexus_amx_context_hash: Hash::new(b"selective test nexus"),
        execution_policy_hash: Hash::new(b"selective test execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x5a; 32],
    };
    let context_id = context.id();
    let subject = BlockSubject {
        parent_block_hash: header.prev_block_hash(),
        block_hash: header.hash(),
        payload_hash: block.canonical_proposal_wire_hash().unwrap(),
    };
    let round = ConsensusRound {
        context_id,
        height: 1,
        view: 0,
    };
    let wire = block.encode_wire().unwrap();
    let execution_commitment = ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        Hash::new(b"parent state"),
        Hash::new(b"post state"),
        Hash::new(b"ordinary writes"),
        wire.len() as u64,
        Hash::new(&wire),
    )
    .with_transaction_commitments_from_block(block)
    .unwrap();
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let preimage = Vote {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(keys[*index as usize].private_key(), &preimage)
                .unwrap()
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&refs).unwrap();
    let artifact = iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact::new(
        context,
        subject,
        commit_qc,
        proofs_of_possession,
    );
    BridgeFinalityBundle {
        commitment: BridgeCommitment {
            network_id: network(),
            height_context_id: artifact.context_id(),
            block_height: 1,
            block_hash: artifact.block_hash,
        },
        finality_proof: BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: header,
            finality_artifact: artifact,
        },
    }
}

fn response(rows: Vec<CommittedTransaction>) -> Vec<u8> {
    norito::to_bytes(&QueryResponse::Iterable(QueryOutput {
        batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::CommittedTransaction(
            rows,
        )),
        remaining_items: Some(0),
        has_more: false,
        continue_cursor: None,
    }))
    .unwrap()
}

#[test]
fn kagemusha_testnet_anchor_requires_signed_consecutive_chain_from_independent_context() {
    let (block, _) = selected_block();
    let bundle = finality_bundle(&block);
    let trusted_context = bundle.commitment.height_context_id;
    let chain = json::to_json(&vec![bundle.clone()]).unwrap();
    let anchor = crate::kagemusha_testnet_finality_chain_v1::
        verify_kagemusha_testnet_finality_anchor_from_chain_v1(
            network(),
            trusted_context,
            chain.as_bytes(),
        )
        .unwrap();
    assert_eq!(anchor.network_id, network());
    assert_eq!(anchor.block_height, 1);
    assert_eq!(anchor.height_context_id, trusted_context);

    let wrong_context = HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed([7; 32])));
    assert!(crate::kagemusha_testnet_finality_chain_v1::
        verify_kagemusha_testnet_finality_anchor_from_chain_v1(
            network(),
            wrong_context,
            chain.as_bytes(),
        )
        .is_err());
    let wrong_network =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([9; 32])));
    assert!(crate::kagemusha_testnet_finality_chain_v1::
        verify_kagemusha_testnet_finality_anchor_from_chain_v1(
            wrong_network,
            trusted_context,
            chain.as_bytes(),
        )
        .is_err());

    #[cfg(unix)]
    assert!(crate::kagemusha_testnet_finality_chain_v1::
        pin_kagemusha_testnet_authenticated_finality_chain_v1(
            [0x71; 32],
            network(),
            trusted_context,
            chain.as_bytes(),
        )
        .is_err()); // A valid chain cannot install a pin without the private native owner.
}

#[test]
fn authentic_current_row_and_four_negative_evidence_cases() {
    let (block, selected) = selected_block();
    let bundle = finality_bundle(&block);
    let root = json::to_value(&bundle.commitment.height_context_id.0)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let chain = json::to_json(&vec![bundle.clone()]).unwrap();
    let selected_response = response(vec![selected.clone()]);
    let verified = verify_committed_transaction_inclusion(
        &selected_response,
        chain.as_bytes(),
        network(),
        &root,
        selected.entrypoint_hash,
    )
    .unwrap();
    assert_eq!(verified.row, norito::to_bytes(&selected).unwrap());
    assert_eq!(verified.output_hash, *selected.output_hash.as_ref());
    assert_eq!(verified.block_hash, *block.hash().as_ref());
    assert_eq!(verified.block_height, 1);
    assert!(verified.result_ok);
    assert_eq!(
        candidate_block_hash(&selected_response, selected.entrypoint_hash).unwrap(),
        Some(verified.block_hash),
    );

    let wrong_network = NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign network")),
    );
    assert!(
        verify_committed_transaction_inclusion(
            &selected_response,
            chain.as_bytes(),
            wrong_network,
            &root,
            selected.entrypoint_hash,
        )
        .is_err()
    );
    let mut altered_proof = bundle.clone();
    altered_proof
        .finality_proof
        .finality_artifact
        .commit_qc
        .execution_commitment
        .executed_block_wire_hash = Hash::new(b"altered unsigned proof");
    let altered_chain = json::to_json(&vec![altered_proof]).unwrap();
    assert!(
        verify_committed_transaction_inclusion(
            &selected_response,
            altered_chain.as_bytes(),
            network(),
            &root,
            selected.entrypoint_hash,
        )
        .is_err()
    );
    let wrong_transaction = HashOf::from_untyped_unchecked(Hash::new(b"wrong transaction"));
    assert!(
        verify_committed_transaction_inclusion(
            &selected_response,
            chain.as_bytes(),
            network(),
            &root,
            wrong_transaction,
        )
        .is_err()
    );
    assert!(candidate_block_hash(&selected_response, wrong_transaction).is_err());
    let mut altered_output = selected.clone();
    altered_output.output_hash = HashOf::from_untyped_unchecked(Hash::new(b"mismatched output"));
    assert!(
        verify_committed_transaction_inclusion(
            &response(vec![altered_output]),
            chain.as_bytes(),
            network(),
            &root,
            selected.entrypoint_hash,
        )
        .is_err()
    );
    let mut rehashed_output = selected.clone();
    rehashed_output.output = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index: 0,
        result: Err(
            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted("replacement result".into()),
            ),
        )
        .into(),
        completions: Vec::new(),
    });
    rehashed_output.output_hash = HashOf::new(&rehashed_output.output);
    assert!(
        verify_committed_transaction_inclusion(
            &response(vec![rehashed_output]),
            chain.as_bytes(),
            network(),
            &root,
            selected.entrypoint_hash,
        )
        .is_err()
    );
}

#[test]
fn candidate_distinguishes_exact_empty_page_from_invalid_evidence() {
    let (_, selected) = selected_block();
    let empty = response(Vec::new());
    assert_eq!(
        candidate_block_hash(&empty, selected.entrypoint_hash).unwrap(),
        None,
    );
    assert!(decode_single_response(&empty).is_err());

    let mut output = [0xff; 32];
    let status = unsafe {
        connect_norito_committed_transaction_candidate_block_hash_v1(
            empty.as_ptr(),
            empty.len() as c_ulong,
            selected.entrypoint_hash.as_ref().as_ptr(),
            32,
            output.as_mut_ptr(),
        )
    };
    assert_eq!(status, 1);
    assert_eq!(output, [0; 32]);

    let multirow = response(vec![selected.clone(), selected.clone()]);
    assert!(candidate_block_hash(&multirow, selected.entrypoint_hash).is_err());
    let mut output = [0xff; 32];
    let status = unsafe {
        connect_norito_committed_transaction_candidate_block_hash_v1(
            multirow.as_ptr(),
            multirow.len() as c_ulong,
            selected.entrypoint_hash.as_ref().as_ptr(),
            32,
            output.as_mut_ptr(),
        )
    };
    assert_eq!(status, ERR_COMMITTED_INCLUSION);
    assert_eq!(output, [0; 32]);
    let foreign = norito::to_bytes(&QueryResponse::Iterable(QueryOutput {
        batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![])),
        remaining_items: Some(0),
        has_more: false,
        continue_cursor: None,
    }))
    .unwrap();
    assert!(candidate_block_hash(&foreign, selected.entrypoint_hash).is_err());
    let inconsistent_page = norito::to_bytes(&QueryResponse::Iterable(QueryOutput {
        batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::CommittedTransaction(
            Vec::new(),
        )),
        remaining_items: Some(1),
        has_more: false,
        continue_cursor: None,
    }))
    .unwrap();
    assert!(candidate_block_hash(&inconsistent_page, selected.entrypoint_hash).is_err());
    assert!(candidate_block_hash(&[0x7f], selected.entrypoint_hash).is_err());
}

#[test]
fn ffi_failure_clears_every_output_before_rejection() {
    let mut row_pointer = 1usize as *mut u8;
    let mut row_len: c_ulong = 12;
    let mut output_hash = [0xff; 32];
    let mut block_hash = [0xff; 32];
    let mut height = 42_u64;
    let mut result_ok = 1_u8;
    let status = unsafe {
        connect_norito_verify_committed_transaction_inclusion_v1(
            ptr::null(),
            0,
            ptr::null(),
            0,
            ptr::null(),
            0,
            ptr::null(),
            0,
            ptr::null(),
            0,
            &mut row_pointer,
            &mut row_len,
            output_hash.as_mut_ptr(),
            block_hash.as_mut_ptr(),
            &mut height,
            &mut result_ok,
        )
    };
    assert_eq!(status, ERR_COMMITTED_INCLUSION);
    assert!(row_pointer.is_null());
    assert_eq!(row_len, 0);
    assert_eq!(output_hash, [0; 32]);
    assert_eq!(block_hash, [0; 32]);
    assert_eq!(height, 0);
    assert_eq!(result_ok, 0);
}
