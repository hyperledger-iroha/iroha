//! Real BLS finality and exact-output regressions for the Python native boundary.
use super::*;
use crate::{PyNetworkId, verify_committed_transaction_inclusion_json_py};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    block::{BlockHeader, consensus_v2::*},
    bridge::{BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeCommitment, BridgeFinalityProof},
    query::{QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryResponse},
    transaction::TransactionBuilder,
};
use std::{num::NonZeroU64, time::Duration};

fn test_network_id() -> NetworkId {
    NetworkId::from_genesis_hash(
        HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(Hash::prehashed(
            [0xA5; Hash::LENGTH],
        )),
    )
}
fn client_fixture_output_limits() -> iroha_data_model::block::output_budget::ExecutionOutputLimits {
    iroha_data_model::block::output_budget::ExecutionOutputLimits {
        max_outputs: 16,
        max_output_bytes: 64 * 1024,
        max_total_output_bytes: 256 * 1024,
        max_executed_wire_bytes: 1024 * 1024,
    }
}
fn client_fixture_network_output(
    input_index: u32,
    result: iroha_data_model::transaction::TransactionResult,
) -> iroha_data_model::block::execution_output::ExecutionOutputV1 {
    use iroha_data_model::block::execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1};
    ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
        input_index,
        result,
        completions: Vec::new(),
    })
}
fn attach_client_fixture_outputs(
    block: &mut SignedBlock,
    outputs: Vec<iroha_data_model::block::execution_output::ExecutionOutputV1>,
    fragments: u64,
) {
    block
        .set_execution_outputs(
            outputs,
            fragments,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &client_fixture_output_limits(),
        )
        .expect("attach bounded canonical client fixture outputs");
}
fn mint_finality_authorization_fixture(
    roster: &[ValidatorPower],
) -> (
    iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1,
    iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1,
) {
    use iroha_data_model::isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationV1,
        KagemushaMintFinalityValidatorKeysV1,
    };

    // Public test-only Pallas/Vesta generator multiples 1..=4, matching
    // iroha_genesis::deterministic_test_kagemusha_mint_finality_genesis_parameters_for
    // and the scalar construction in iroha_sccp::test_fixtures. These are
    // independently provisioned fixture keys, never derived from BLS keys.
    const EQ_PROOF_PUBLIC_KEYS: [&str; 4] = [
        "00000000ed302d991bf94c09fc98462200000000000000000000000000000040",
        "030000b067c50313fcac1144eee2fe0e0000000000000000000000000000001c",
        "63d232eb3b8af0b75cfcf55ade47f6ff4cdf4e47a7454cb8ed67a9ba6f56e788",
        "fc86bc8efbbcb878f49427618b6940409b9157e3d777a4c4c0514a8e0d92db18",
    ];
    const EP_PROOF_PUBLIC_KEYS: [&str; 4] = [
        "0000000021eb468cdda89409fc98462200000000000000000000000000000040",
        "03000070de065fede0093144eee2fe0e0000000000000000000000000000001c",
        "5fce556feb6fee5a15560ddabae10224b026a5d0281af4c613955c39a8797837",
        "f79037a77e26a2c0794dc326d866c664616499c064073a8f8ebf3080297be5ab",
    ];
    assert_eq!(roster.len(), 4, "fixture has exactly four validators");
    let authority = KagemushaMintFinalityAuthorityGenerationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: test_network_id(),
        generation: 0,
        validators: roster
            .iter()
            .enumerate()
            .map(|(index, validator)| {
                let mut pallas_key = [0; 32];
                hex::decode_to_slice(EQ_PROOF_PUBLIC_KEYS[index], &mut pallas_key)
                    .expect("valid fixed Pallas fixture key");
                let mut vesta_key = [0; 32];
                hex::decode_to_slice(EP_PROOF_PUBLIC_KEYS[index], &mut vesta_key)
                    .expect("valid fixed Vesta fixture key");
                KagemushaMintFinalityValidatorKeysV1 {
                    validator: validator.validator.clone(),
                    eq_proof_public_key: pallas_key,
                    ep_proof_public_key: vesta_key,
                }
            })
            .collect(),
    };
    let authorization = {
        let authority = &authority;
        let authorization = iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1 {
                version: iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
                network_id: authority.network_id,
                epoch: 0,
                first_height: 1,
                last_height: 10,
                authority_generation: authority.generation,
                authority_id: authority.authority_id().expect("fixture authority identity"),
                beacon: iroha_data_model::isi::kagemusha_v1::BeaconEpochBindingV1::Bootstrap,
                previous_authorization_id: [0; 32],
                transition_id: [0; 32],
                decision: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochDecisionV1::Genesis,
            };
        authorization
            .validate_against_authority(authority)
            .expect("complete genesis fixture authorization");
        authorization
    };
    (authorization, authority)
}
fn canonical_executed_network_fixture(
    inputs: u32,
    include_internal: bool,
    selected: u32,
    block_height: u64,
    parent: Option<HashOf<BlockHeader>>,
) -> (NonZeroU64, SignedBlock, CommittedTransaction) {
    use iroha_crypto::{PrivateKey, PublicKey};
    use iroha_data_model::block::{
        builder::BlockBuilder,
        execution_output::{ExecutionOutputV1, TimeInvocationV1, TriggerUseV1},
    };
    use iroha_data_model::events::time::{TimeEvent, TimeInterval};
    let public_key: PublicKey =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .unwrap();
    let private_key: PrivateKey =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
            .parse()
            .unwrap();
    let authority = AccountId::new(public_key);
    let height = NonZeroU64::new(block_height).unwrap();
    let header = BlockHeader::new(height, parent, None, 10 + block_height, 0);
    let mut builder = BlockBuilder::new(header);
    for index in 0..inputs {
        let mut transaction = TransactionBuilder::new(
            test_network_id(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        transaction.set_creation_time(Duration::from_millis(u64::from(index) + 1));
        builder.push_transaction(transaction.try_sign(&private_key).unwrap());
    }
    let mut block = builder.try_build_with_signature(0, &private_key).unwrap();
    let mut outputs = (0..inputs)
        .map(|index| {
            client_fixture_network_output(
                index,
                Ok(iroha_data_model::transaction::DataTriggerSequence::default()).into(),
            )
        })
        .collect::<Vec<_>>();
    if include_internal {
        let invocation = TimeInvocationV1 {
            schedule_index: 0,
            event: TimeEvent {
                interval: TimeInterval {
                    since_ms: 9,
                    length_ms: 1,
                },
            },
            trigger: TriggerUseV1 {
                trigger_id: "client-internal-output".parse().unwrap(),
                registered_at_height: 0,
                action_hash: Hash::new(b"client exact action fixture"),
            },
        };
        outputs.push(ExecutionOutputV1::Time(
            iroha_data_model::block::execution_output::TimeExecutionOutputV1 {
                result: Ok(vec![iroha_data_model::trigger::DataTriggerStep {
                    id: invocation.trigger.trigger_id.clone(),
                    instructions: iroha_data_model::transaction::ExecutionStep(Vec::new().into()),
                }])
                .into(),
                invocation,
                failure_root: None,
                completions: Vec::new(),
            },
        ));
    }
    attach_client_fixture_outputs(&mut block, outputs, u64::from(inputs));
    let (output_index, row) = block.network_output_at(selected).unwrap();
    let output = ExecutionOutputV1::Network(row.clone());
    let committed = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: block
            .network_entrypoint_at(selected as usize)
            .unwrap()
            .hash(),
        entrypoint_proof: block.network_input_proof(selected).unwrap(),
        entrypoint: block
            .network_entrypoint_at(selected as usize)
            .unwrap()
            .clone(),
        output_hash: HashOf::new(&output),
        output_proof: block.output_proof(output_index).unwrap(),
        output,
    };
    assert!(committed.verify_inclusion_in_block(&block));
    if include_internal {
        assert_eq!(
            block
                .network_input_merkle_commitment()
                .unwrap()
                .leaf_count()
                .get(),
            u64::from(inputs)
        );
        assert_eq!(
            block.output_merkle_commitment().unwrap().leaf_count().get(),
            u64::from(inputs) + 1
        );
    }
    (height, block, committed)
}
fn synthetic_executed_commitment(
    block: &SignedBlock,
) -> iroha_data_model::block::consensus_v2::ExecutionCommitment {
    let wire = block.encode_wire().expect("fixture executed wire");
    // The HTTP tests supply a trust input; they do not claim consensus qualification.
    iroha_data_model::block::consensus_v2::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
        Hash::new(b"fixture parent state"), Hash::new(b"fixture post state"),
        Hash::new(b"fixture ordinary writes"), wire.len() as u64, Hash::new(&wire),
    )
}
fn sign_bridge_finality_qc(commit_qc: &mut QuorumCertificate, keys: &[KeyPair]) {
    let preimage = Vote {
        round: commit_qc.round,
        proposal_round: commit_qc.proposal_round,
        phase: commit_qc.phase,
        subject: commit_qc.subject,
        execution_commitment: commit_qc.execution_commitment,
        signer: commit_qc.signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let signature_payloads = commit_qc
        .signers
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).expect("fixture signer index");
            Signature::try_new(keys[index].private_key(), &preimage)
                .expect("sign finality fixture vote")
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signature_payloads
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate finality fixture votes");
}
fn keys() -> Vec<KeyPair> {
    let mut keys = (0..4)
        .map(|_| {
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                .expect("generate BLS finality fixture key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        iroha_model_base::peer::PeerId::new(left.public_key().clone()).cmp(
            &iroha_model_base::peer::PeerId::new(right.public_key().clone()),
        )
    });
    keys
}
fn proof_for(
    block: &SignedBlock,
    parent: Option<&BridgeFinalityProof>,
    keys: &[KeyPair],
) -> BridgeFinalityProof {
    let roster = keys
        .iter()
        .map(|key| ValidatorPower {
            validator: iroha_model_base::peer::PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let proofs_of_possession = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("derive finality fixture proof of possession")
        })
        .collect::<Vec<_>>();
    let height = block.header().height();
    let header = block.header();
    let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
        mint_finality_authorization_fixture(&roster);
    let context = HeightContext {
        network_id: test_network_id(),
        protocol_version: PROTOCOL_VERSION,
        height: height.get(),
        epoch: 0,
        kagemusha_mint_finality_authorization,
        kagemusha_mint_finality_authority,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: parent.map(|proof| proof.finality_artifact.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid finality fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"client finality fixture nexus context"),
        execution_policy_hash: Hash::new(b"client finality fixture execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x5A; Hash::LENGTH],
    };
    let context_id = context.id();
    let subject = BlockSubject {
        parent_block_hash: header.prev_block_hash(),
        block_hash: header.hash(),
        payload_hash: block.canonical_proposal_wire_hash().unwrap(),
    };
    let round = ConsensusRound {
        context_id,
        height: height.get(),
        view: 0,
    };
    let execution_commitment = synthetic_executed_commitment(block);
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    sign_bridge_finality_qc(&mut commit_qc, keys);
    let finality_artifact =
        iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact::new(
            context,
            subject,
            commit_qc,
            proofs_of_possession,
        );
    let proof = BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: header,
        finality_artifact,
    };
    proof
}

fn bundle(proof: BridgeFinalityProof) -> BridgeFinalityBundle {
    let artifact = &proof.finality_artifact;
    BridgeFinalityBundle {
        commitment: BridgeCommitment {
            network_id: artifact.height_context.network_id,
            height_context_id: artifact.context_id(),
            block_height: artifact.height,
            block_hash: artifact.block_hash,
        },
        finality_proof: proof,
    }
}
fn scalar<T: json::JsonSerialize>(value: &T) -> String {
    json::to_value(value).unwrap().as_str().unwrap().to_owned()
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
fn verify(
    committed: &CommittedTransaction,
    wire: &[u8],
    bundles: &[BridgeFinalityBundle],
    root: &str,
) -> PyResult<String> {
    verify_committed_transaction_inclusion_json_py(
        &hex::encode(committed.entrypoint_hash.as_ref()),
        &response(vec![committed.clone()]),
        wire,
        &json::to_json(&bundles.to_vec()).unwrap(),
        &PyNetworkId {
            inner: test_network_id(),
        },
        root,
    )
}

#[test]
fn native_selected_output_authenticates_real_bls_chain_and_exact_projection() {
    pyo3::Python::initialize();
    let keys = keys();
    let (_, first, _) = canonical_executed_network_fixture(2, true, 1, 1, None);
    let first_proof = proof_for(&first, None, &keys);
    let root = scalar(&first_proof.finality_artifact.context_id().0);
    let (_, next, selected) = canonical_executed_network_fixture(2, true, 1, 2, Some(first.hash()));
    let next_proof = proof_for(&next, Some(&first_proof), &keys);
    let bundles = vec![bundle(first_proof), bundle(next_proof)];
    let wire = next.encode_wire().unwrap();
    let result: json::Value =
        json::from_json(&verify(&selected, &wire, &bundles, &root).unwrap()).unwrap();
    assert_eq!(
        result["output_hash"].as_str(),
        Some(hex::encode(selected.output_hash.as_ref()).as_str())
    );
    assert_eq!(
        result["block_hash"].as_str(),
        Some(hex::encode(next.hash().as_ref()).as_str())
    );
    assert_eq!(result["block_height"].as_u64(), Some(2));
    assert_eq!(
        result["executed_block_wire_len"].as_u64(),
        Some(wire.len() as u64)
    );
    assert_eq!(
        result["executed_block_wire_hash"].as_str(),
        Some(hex::encode(Hash::new(&wire).as_ref()).as_str())
    );
    assert_eq!(
        result["network_id"],
        json::to_value(&test_network_id()).unwrap()
    );
    assert_eq!(
        result["height_context_id"],
        json::to_value(&bundles[1].commitment.height_context_id.0).unwrap()
    );
    assert_eq!(
        result["execution_commitment"],
        json::to_value(
            &bundles[1]
                .finality_proof
                .finality_artifact
                .commit_qc
                .execution_commitment
        )
        .unwrap()
    );
    assert_eq!(result["result_ok"].as_bool(), Some(true));
    let child_root = scalar(&bundles[1].commitment.height_context_id.0);
    assert!(verify(&selected, &wire, &bundles[1..], &child_root).is_ok());
    for bad in [
        bundles[1..].to_vec(),
        vec![bundles[1].clone(), bundles[0].clone()],
        vec![bundles[0].clone(), bundles[0].clone()],
    ] {
        assert!(verify(&selected, &wire, &bad, &root).is_err());
    }
    let mut bad = bundles.clone();
    bad[1]
        .finality_proof
        .finality_artifact
        .commit_qc
        .execution_commitment
        .executed_block_wire_hash = Hash::new(b"unsigned replacement commitment");
    assert!(verify(&selected, &wire, &bad, &root).is_err());
    bad = bundles.clone();
    bad[1].commitment.block_height += 1;
    assert!(verify(&selected, &wire, &bad, &root).is_err());
    assert!(
        verify(
            &selected,
            &wire,
            &bundles,
            &scalar(&Hash::new(b"foreign root"))
        )
        .is_err()
    );
    let other_network = PyNetworkId::from_exact_bytes(&[0xA7; 32]).unwrap();
    assert!(
        verify_committed_transaction_inclusion_json_py(
            &hex::encode(selected.entrypoint_hash.as_ref()),
            &response(vec![selected.clone()]),
            &wire,
            &json::to_json(&bundles).unwrap(),
            &other_network,
            &root
        )
        .is_err()
    );
    assert!(
        verify_committed_transaction_inclusion_json_py(
            &hex::encode(Hash::new(b"other transaction").as_ref()),
            &response(vec![selected]),
            &wire,
            &json::to_json(&bundles).unwrap(),
            &PyNetworkId {
                inner: test_network_id()
            },
            &root
        )
        .is_err()
    );
}

#[test]
fn native_selected_output_rejects_rehashed_outputs_and_swapped_rows() {
    pyo3::Python::initialize();
    let (_, original, selected) = canonical_executed_network_fixture(2, true, 1, 1, None);
    let keys = keys();
    let proof = proof_for(&original, None, &keys);
    let root = scalar(&proof.finality_artifact.context_id().0);
    let bundles = vec![bundle(proof)];
    let wire = original.encode_wire().unwrap();
    let mut rewritten = original.clone();
    let mut rows = rewritten.execution_outputs().to_vec();
    rows[1] = client_fixture_network_output(
        1,
        Err(
            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted("substituted result".into()),
            ),
        )
        .into(),
    );
    attach_client_fixture_outputs(&mut rewritten, rows, 2);
    let mut changed = selected.clone();
    changed.output = rewritten.execution_outputs()[1].clone();
    changed.output_hash = HashOf::new(&changed.output);
    changed.output_proof = rewritten.output_proof(1).unwrap();
    assert_eq!(rewritten.header(), original.header());
    assert!(changed.verify_inclusion_in_block(&rewritten));
    assert!(verify(&changed, &rewritten.encode_wire().unwrap(), &bundles, &root).is_err());
    // Rejected execution is still valid evidence when the actual QC signs that wire.
    let rejection_bundle = vec![bundle(proof_for(&rewritten, None, &keys))];
    let rejected: json::Value = json::from_json(
        &verify(
            &changed,
            &rewritten.encode_wire().unwrap(),
            &rejection_bundle,
            &root,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(rejected["result_ok"].as_bool(), Some(false));
    for index in [0, 2] {
        let mut swapped = selected.clone();
        swapped.output = original.execution_outputs()[index].clone();
        swapped.output_hash = HashOf::new(&swapped.output);
        swapped.output_proof = original.output_proof(index as u32).unwrap();
        assert!(verify(&swapped, &wire, &bundles, &root).is_err());
    }
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(verify(&selected, &trailing, &bundles, &root).is_err());
    assert!(
        verify_committed_transaction_inclusion_json_py(
            &hex::encode(selected.entrypoint_hash.as_ref()),
            &response(vec![selected.clone(), selected.clone()]),
            &wire,
            &json::to_json(&bundles).unwrap(),
            &PyNetworkId {
                inner: test_network_id()
            },
            &root
        )
        .is_err()
    );
    assert!(verify(&selected, &wire, &[], &root).is_err());
    assert!(
        verify(
            &selected,
            &wire,
            &bundles,
            &hex::encode(bundles[0].commitment.height_context_id.0.as_ref())
        )
        .is_err()
    );
}

#[test]
fn native_selected_output_rejects_chain_resource_overflow_before_authentication() {
    pyo3::Python::initialize();
    let expected = HashOf::from_untyped_unchecked(Hash::new(b"selected"));
    let root = scalar(&Hash::new(b"root"));
    assert!(
        authenticate_committed_transaction(
            expected,
            &[],
            &[],
            &" ".repeat(MAX_FINALITY_CHAIN_JSON_BYTES + 1),
            test_network_id(),
            &root
        )
        .unwrap_err()
        .to_string()
        .contains("exceeds 16 MiB")
    );
    let (_, block, selected) = canonical_executed_network_fixture(1, false, 0, 1, None);
    let proof = proof_for(&block, None, &keys());
    let root = scalar(&proof.finality_artifact.context_id().0);
    let bundles = vec![bundle(proof); MAX_FINALITY_CHAIN_BUNDLES + 1];
    let error = verify(&selected, &block.encode_wire().unwrap(), &bundles, &root)
        .unwrap_err()
        .to_string();
    assert!(error.contains("1..4096") || error.contains("exceeds 16 MiB"));
}
