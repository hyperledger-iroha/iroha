// Structural HTTP fixtures; execution/finality authority is established separately below.
fn canonical_executed_block_fixture() -> (NonZeroU64, SignedBlock, CommittedTransaction) {
    canonical_executed_network_fixture(1, false, 0)
}

fn canonical_executed_network_fixture(
    inputs: u32,
    include_internal: bool,
    selected: u32,
) -> (NonZeroU64, SignedBlock, CommittedTransaction) {
    use crate::crypto::{PrivateKey, PublicKey};
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
    let height = NonZeroU64::new(1).unwrap();
    let header = BlockHeader::new(height, None, None, 10, 0);
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
    .with_transaction_commitments_from_block(block)
    .expect("fixture exact input/output commitments")
}

#[test]
fn canonical_executed_block_reader_requires_authenticated_execution_commitment() {
    let client = client_with_base_url(base_url());
    for (height, block, committed) in [
        canonical_executed_block_fixture(),
        canonical_executed_network_fixture(2, true, 1),
    ] {
        let expected = synthetic_executed_commitment(&block);
        let wire = block.encode_wire().expect("fixture wire");
        let response = capture_request(
            mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
            |transport| {
                let client = client.clone().with_test_http_transport(transport);
                mark_data_model_compatible(&client);
                client.get_canonical_executed_block_wire(height, &committed, &expected)
            },
        )
        .0;
        assert_eq!(
            response.expect("exact Network sources and separate output trees verify"),
            wire
        );
        for wrong_length in [false, true] {
            let mut wrong = expected.clone();
            if wrong_length {
                wrong.executed_block_wire_len += 1;
            } else {
                wrong.executed_block_wire_hash = Hash::new(b"wrong wire");
            }
            let response = capture_request(
                mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
                |transport| {
                    let client = client.clone().with_test_http_transport(transport);
                    mark_data_model_compatible(&client);
                    client.get_canonical_executed_block_wire(height, &committed, &wrong)
                },
            )
            .0;
            assert!(
                response
                    .expect_err("wrong exact execution commitment must fail")
                    .to_string()
                    .contains("authenticated execution commitment")
            );
        }
    }
    let (height, accepted, committed) = canonical_executed_block_fixture();
    let mut actually_rejected = accepted.clone();
    attach_client_fixture_outputs(
        &mut actually_rejected,
        vec![client_fixture_network_output(
            0,
            Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted(
                        "authenticated rejected execution".into(),
                    ),
                ),
            )
            .into(),
        )],
        0,
    );
    assert_eq!(accepted.header(), actually_rejected.header());
    assert_eq!(
        accepted.hash(),
        actually_rejected.hash(),
        "outputs do not rewrite the proposal"
    );
    assert_ne!(
        accepted.output_merkle_commitment(),
        actually_rejected.output_merkle_commitment()
    );
    let expected = synthetic_executed_commitment(&actually_rejected);
    let response = capture_request(
        mk_response(
            StatusCode::OK,
            accepted.encode_wire().unwrap(),
            Some(APPLICATION_NORITO),
        ),
        |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_canonical_executed_block_wire(height, &committed, &expected)
        },
    )
    .0;
    assert!(
        response
            .expect_err("forged successful result with unchanged consensus hash must fail")
            .to_string()
            .contains("authenticated execution commitment")
    );
}

#[test]
fn canonical_executed_block_reader_rejects_a_transaction_from_another_network() {
    let (height, block, committed) = canonical_executed_block_fixture();
    let wire = block.encode_wire().expect("canonical executed block wire");
    let commitment = synthetic_executed_commitment(&block);
    let mut client = client_with_base_url(base_url());
    client.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"another network for executed block reader",
    )));
    let response = capture_request(
        mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)),
        |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_canonical_executed_block_wire(height, &committed, &commitment)
        },
    )
    .0;
    assert!(
        response
            .expect_err("a valid committed transaction from another network must fail")
            .to_string()
            .contains("authenticated client network")
    );
}

#[test]
fn canonical_executed_block_reader_binds_route_wire_and_committed_evidence() {
    let mut client = client_with_base_url(base_url());

    client
        .headers
        .insert("Accept".to_owned(), APPLICATION_JSON.to_owned());
    client
        .headers
        .insert("Content-Type".to_owned(), APPLICATION_JSON.to_owned());
    let (height, block, committed) = canonical_executed_block_fixture();
    let wire = block.encode_wire().expect("canonical executed block wire");
    let (actual, snapshot) = capture_request(
        mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_canonical_executed_block_wire(
                height,
                &committed,
                &synthetic_executed_commitment(&block),
            )
        },
    );
    assert_eq!(actual.expect("verified executed block wire"), wire);
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/ledger/block/1");
    assert!(snapshot.url.query().is_none());
    assert!(snapshot.body.is_empty());
    super::tests::assert_canonical_account_signed_request(&client, &snapshot);
    assert_eq!(
        snapshot.max_response_bytes,
        AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1
    );
    assert_single_accept_header(&snapshot, APPLICATION_NORITO);
    assert!(
        snapshot
            .headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("content-type")),
        "GET must not carry Content-Type: {:?}",
        snapshot.headers
    );
}

#[test]
fn canonical_executed_block_reader_retries_with_fresh_account_signature() {
    let client = client_with_base_url(base_url());
    let (height, block, committed) = canonical_executed_block_fixture();
    let commitment = synthetic_executed_commitment(&block);
    let wire = block.encode_wire().expect("canonical executed block wire");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&requests);
    let result = with_mock_http(
        move |request| {
            let mut requests = observed.lock().expect("captured requests");
            requests.push(request);
            if requests.len() == 1 {
                let mut response = empty_response(StatusCode::TOO_MANY_REQUESTS);
                response
                    .headers_mut()
                    .insert("retry-after", "0".parse().unwrap());
                Ok(response)
            } else {
                Ok(mk_response(
                    StatusCode::OK,
                    wire.clone(),
                    Some(APPLICATION_NORITO),
                ))
            }
        },
        |transport| {
            let client = client
                .clone()
                .with_test_http_transport(transport)
                .with_request_deadline(std::time::Instant::now() + Duration::from_secs(5));
            mark_data_model_compatible(&client);
            client.get_canonical_executed_block_wire(height, &committed, &commitment)
        },
    );
    assert_eq!(
        result.expect("signed retry verifies canonical wire"),
        block.encode_wire().unwrap()
    );
    let requests = requests.lock().expect("captured requests");
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        super::tests::assert_canonical_account_signed_request(&client, request);
        assert_single_accept_header(request, APPLICATION_NORITO);
        assert_eq!(
            request.max_response_bytes,
            AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1
        );
    }
    let nonce = |request: &RequestSnapshot| {
        request
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(HEADER_NONCE))
            .expect("canonical request nonce")
            .1
            .clone()
    };
    assert_ne!(nonce(&requests[0]), nonce(&requests[1]));
}

#[test]
fn canonical_executed_block_reader_rejects_trailing_wire_and_wrong_carrier_hash() {
    let client = client_with_base_url(base_url());

    let (height, block, committed) = canonical_executed_block_fixture();
    let mut trailing = block.encode_wire().expect("canonical executed block wire");
    trailing.push(0);
    let error = capture_request(
        mk_response(StatusCode::OK, trailing, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_canonical_executed_block_wire(
                height,
                &committed,
                &synthetic_executed_commitment(&block),
            )
        },
    )
    .0
    .expect_err("trailing executed-block bytes must fail");
    assert!(
        error
            .to_string()
            .contains("decode canonical executed block wire")
            || error
                .to_string()
                .contains("exact canonical SignedBlock wire")
    );

    let mut wrong_carrier = committed;
    wrong_carrier.block_hash =
        HashOf::from_untyped_unchecked(Hash::prehashed([0x91; Hash::LENGTH]));
    let wire = block.encode_wire().expect("canonical executed block wire");
    let error = capture_request(
        mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_canonical_executed_block_wire(
                height,
                &wrong_carrier,
                &synthetic_executed_commitment(&block),
            )
        },
    )
    .0
    .expect_err("wrong carrier hash must fail");
    assert!(error.to_string().contains("carrier hash"));
}

#[test]
fn canonical_executed_block_reader_rejects_rehashed_source_and_internal_substitutions() {
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    let client = client_with_base_url(base_url());
    let (height, block, committed) = canonical_executed_network_fixture(2, true, 1);
    let commitment = synthetic_executed_commitment(&block);
    let wire = block.encode_wire().unwrap();
    for mutation in 0..5 {
        let mut changed = committed.clone();
        match mutation {
            0 => {
                let ExecutionOutputV1::Network(row) = &mut changed.output else {
                    unreachable!()
                };
                row.input_index = 0;
            }
            1 => changed.entrypoint_proof = block.network_input_proof(0).unwrap(),
            2 => {
                changed.entrypoint = block.network_entrypoint_at(0).unwrap().clone();
                changed.entrypoint_hash = changed.entrypoint.hash();
                changed.entrypoint_proof = block.network_input_proof(0).unwrap();
            }
            3 => {
                changed.output = block.execution_outputs()[2].clone();
                changed.output_proof = block.output_proof(2).unwrap();
                assert!(
                    changed.result().is_ok(),
                    "internal success is still no Network result"
                );
            }
            4 => changed.output_proof = block.output_proof(0).unwrap(),
            _ => unreachable!(),
        }
        changed.output_hash = HashOf::new(&changed.output);
        let error = capture_request(
            mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
            |transport| {
                let client = client.clone().with_test_http_transport(transport);
                mark_data_model_compatible(&client);
                client.get_canonical_executed_block_wire(height, &changed, &commitment)
            },
        )
        .0
        .expect_err(
            "a complete self-consistent claim must still have exact authenticated inclusion",
        );
        assert!(
            error
                .to_string()
                .contains("authenticated execution commitment"),
            "mutation {mutation}: {error}"
        );
    }
}

#[test]
fn canonical_executed_block_reader_joins_real_bls_finality_to_exact_output_wire() {
    let client = client_with_base_url(base_url());
    let (height, block, committed) = canonical_executed_network_fixture(2, true, 1);
    let (proof, _, mut verifier) = bridge_finality_chain_fixture_for_block(Some(&block));
    assert_eq!(proof.finality_artifact.height_context.roster.len(), 4);
    assert_eq!(proof.finality_artifact.commit_qc.signers.len(), 3);
    assert_eq!(proof.finality_artifact.validator_set_pops.len(), 4);
    let authenticated = capture_request(
        mk_response(
            StatusCode::OK,
            norito::to_bytes(&proof).unwrap(),
            Some(APPLICATION_NORITO),
        ),
        |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_bridge_finality_proof(height, block.hash(), &mut verifier)
        },
    )
    .0
    .expect("actual BLS+PoP finality verifies against the independently pinned context");
    assert_eq!(authenticated.block_header.hash(), block.hash());
    let commitment = &authenticated
        .finality_artifact
        .commit_qc
        .execution_commitment;
    let wire = block.encode_wire().unwrap();
    let verified = capture_request(
        mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO)),
        |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_canonical_executed_block_wire(height, &committed, commitment)
        },
    )
    .0
    .expect("exact output wire is bound by the verified commitment");
    assert_eq!(verified, wire);
    let mut altered = block.clone();
    let mut outputs = altered.execution_outputs().to_vec();
    let iroha_data_model::block::execution_output::ExecutionOutputV1::Network(row) =
        &mut outputs[1]
    else {
        unreachable!()
    };
    row.completions.push(
        iroha_data_model::block::execution_output::InvocationCompletionV1 {
            callback_index: 0,
            trigger_id: "substituted-completion".parse().unwrap(),
            outcome: iroha_data_model::events::trigger_completed::TriggerCompletedOutcome::Success,
        },
    );
    attach_client_fixture_outputs(&mut altered, outputs, 2);
    assert_eq!(altered.header(), block.header());
    let (_, altered_row) = altered.network_output_at(1).unwrap();
    let mut changed = committed.clone();
    changed.output =
        iroha_data_model::block::execution_output::ExecutionOutputV1::Network(altered_row.clone());
    changed.output_hash = HashOf::new(&changed.output);
    changed.output_proof = altered.output_proof(1).unwrap();
    assert!(
        changed.verify_inclusion_in_block(&altered),
        "alteration is structurally self-consistent"
    );
    let error = capture_request(
        mk_response(
            StatusCode::OK,
            altered.encode_wire().unwrap(),
            Some(APPLICATION_NORITO),
        ),
        |transport| {
            let client = client.clone().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.get_canonical_executed_block_wire(height, &changed, commitment)
        },
    )
    .0
    .expect_err("rehashed full output cannot replace signed executed wire");
    assert!(
        error
            .to_string()
            .contains("authenticated execution commitment")
    );
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

fn bridge_finality_chain_fixture() -> (
    BridgeFinalityProof,
    BridgeFinalityProof,
    BridgeFinalityVerifier,
) {
    bridge_finality_chain_fixture_for_block(None)
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture builds a self-contained cryptographically valid v2 proof chain"
)]
fn bridge_finality_chain_fixture_for_block(
    block: Option<&SignedBlock>,
) -> (
    BridgeFinalityProof,
    BridgeFinalityProof,
    BridgeFinalityVerifier,
) {
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
    let height = NonZeroU64::new(1).expect("non-zero finality height");
    let header = block.map_or_else(
        || BlockHeader::new(height, None, None, 0, 0),
        SignedBlock::header,
    );
    assert_eq!(header.height(), height);
    let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
        mint_finality_authority_fixture(&roster);
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
        parent_commit_qc: None,
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
        parent_block_hash: None,
        block_hash: header.hash(),
        payload_hash: block.map_or_else(
            || Hash::new(b"client finality fixture payload"),
            |block| block.canonical_proposal_wire_hash().unwrap(),
        ),
    };
    let round = ConsensusRound {
        context_id,
        height: height.get(),
        view: 0,
    };
    let execution_commitment = block.map_or_else(
        || {
            ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                Hash::new(b"client finality fixture parent state"),
                Hash::new(b"client finality fixture post state"),
                Hash::new(b"client finality fixture ordinary writes"),
                1,
                Hash::new(b"client finality fixture executed wire"),
            )
        },
        synthetic_executed_commitment,
    );
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    sign_bridge_finality_qc(&mut commit_qc, &keys);
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
    let parent_artifact = &proof.finality_artifact;
    let successor_height = NonZeroU64::new(height.get() + 1).expect("non-zero successor height");
    let successor_header = BlockHeader::new(
        successor_height,
        Some(parent_artifact.block_hash),
        None,
        header.creation_time_ms + 1,
        0,
    );
    let successor_context = HeightContext {
        network_id: parent_artifact.height_context.network_id,
        protocol_version: PROTOCOL_VERSION,
        height: successor_height.get(),
        epoch: parent_artifact.height_context.epoch,
        kagemusha_mint_finality_authorization: parent_artifact
            .height_context
            .kagemusha_mint_finality_authorization,
        kagemusha_mint_finality_authority: parent_artifact
            .height_context
            .kagemusha_mint_finality_authority
            .clone(),
        epoch_end_height: parent_artifact.height_context.epoch_end_height,
        next_epoch_snapshot: None,
        mode: parent_artifact.height_context.mode,
        parent_commit_qc: Some(parent_artifact.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum: parent_artifact.height_context.quorum,
        roster: parent_artifact.height_context.roster.clone(),
        nexus_amx_context_hash: Hash::new(b"client finality successor nexus context"),
        execution_policy_hash: parent_artifact.height_context.execution_policy_hash,
        da_layout: parent_artifact.height_context.da_layout,
        leader_seed: parent_artifact.height_context.leader_seed,
    };
    let successor_subject = BlockSubject {
        parent_block_hash: Some(parent_artifact.block_hash),
        block_hash: successor_header.hash(),
        payload_hash: Hash::new(b"client finality successor payload"),
    };
    let successor_round = ConsensusRound {
        context_id: successor_context.id(),
        height: successor_height.get(),
        view: 0,
    };
    let successor_execution_commitment =
        ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"client finality successor parent state"),
            Hash::new(b"client finality successor post state"),
            Hash::new(b"client finality successor ordinary writes"),
            1,
            Hash::new(b"client finality successor executed wire"),
        );
    let mut successor_commit_qc = QuorumCertificate {
        round: successor_round,
        proposal_round: successor_round,
        phase: GlobalPhase::Commit,
        subject: successor_subject,
        execution_commitment: successor_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    sign_bridge_finality_qc(&mut successor_commit_qc, &keys);
    let successor_artifact =
        iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact::new(
            successor_context,
            successor_subject,
            successor_commit_qc,
            parent_artifact.validator_set_pops.clone(),
        );
    let successor = BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header: successor_header,
        finality_artifact: successor_artifact,
    };
    let verifier = BridgeFinalityVerifier::with_context(test_network_id(), context_id);
    (proof, successor, verifier)
}

fn rejected_next_bridge_finality_response(
    client: &Client,
    height: NonZeroU64,
    verifier: &mut BridgeFinalityVerifier,
    response: HttpResponse<Vec<u8>>,
) -> String {
    capture_request(response, |mock_transport| {
        let client = client
            .clone()
            .with_test_http_transport(mock_transport.clone());
        mark_data_model_compatible(&client);

        client.get_next_bridge_finality_proof(height, verifier)
    })
    .0
    .expect_err("bridge finality response must fail")
    .to_string()
}

#[test]
fn bridge_finality_anchor_reader_returns_standalone_verified_proof_and_hash() {
    let mut client = client_with_base_url(base_url());

    client
        .headers
        .insert("Accept".to_owned(), APPLICATION_JSON.to_owned());
    client
        .headers
        .insert("Content-Type".to_owned(), APPLICATION_JSON.to_owned());
    let (proof, _, _) = bridge_finality_chain_fixture();
    let expected_hash = proof.block_header.hash();
    let body = norito::to_bytes(&proof).expect("encode canonical bridge finality proof");
    let (actual, snapshot) = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_bridge_finality_anchor(proof.block_header.height(), test_network_id())
        },
    );
    let (actual_proof, actual_hash) = actual.expect("standalone finality proof must verify");
    assert_eq!(actual_proof, proof);
    assert_eq!(actual_hash, expected_hash);
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/1");
    assert!(snapshot.url.query().is_none());
    assert_eq!(
        snapshot.max_response_bytes,
        BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES
    );
    assert_single_accept_header(&snapshot, APPLICATION_NORITO);
    assert!(
        snapshot
            .headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("content-type")),
        "GET must not carry Content-Type: {:?}",
        snapshot.headers
    );
}

#[test]
fn bridge_finality_anchor_reader_rejects_wrong_network_and_invalid_signature() {
    let client = client_with_base_url(base_url());

    let (proof, _, _) = bridge_finality_chain_fixture();
    let body = norito::to_bytes(&proof).expect("encode canonical bridge finality proof");
    let mismatched_height =
        NonZeroU64::new(proof.block_header.height().get() + 1).expect("non-zero mismatched height");
    let (error, snapshot) = capture_request(
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_bridge_finality_anchor(mismatched_height, test_network_id())
        },
    );
    assert!(
        error
            .expect_err("wrong requested anchor height must fail")
            .to_string()
            .contains("requested height")
    );
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/2");

    let wrong_network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"wrong client finality network",
    )));
    let error = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_bridge_finality_anchor(proof.block_header.height(), wrong_network_id)
        },
    )
    .0
    .expect_err("wrong finality network must fail");
    assert!(error.to_string().contains("network"));

    let mut invalid = proof.clone();
    invalid.finality_artifact.commit_qc.aggregate_signature[0] ^= 0x80;
    let body = norito::to_bytes(&invalid).expect("encode invalid bridge finality proof");
    let error = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_bridge_finality_anchor(proof.block_header.height(), test_network_id())
        },
    )
    .0
    .expect_err("invalid finality signature must fail");
    assert!(error.to_string().contains("verification failed"));
}

#[test]
fn bridge_finality_reader_checks_requested_binding_before_advancing_anchor() {
    let mut client = client_with_base_url(base_url());

    client
        .headers
        .insert("Accept".to_owned(), APPLICATION_JSON.to_owned());
    client
        .headers
        .insert("Content-Type".to_owned(), APPLICATION_JSON.to_owned());
    let (proof, _, mut verifier) = bridge_finality_chain_fixture();
    let body = norito::to_bytes(&proof).expect("encode canonical bridge finality proof");
    let wrong_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0x92; Hash::LENGTH]));
    let (error, snapshot) = capture_request(
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);

            client
                .get_bridge_finality_proof(proof.block_header.height(), wrong_hash, &mut verifier)
                .expect_err("wrong requested hash must fail")
        },
    );
    assert!(error.to_string().contains("requested block hash"));
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/1");
    assert!(snapshot.url.query().is_none());
    assert_eq!(
        snapshot.max_response_bytes,
        BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES
    );
    assert_single_accept_header(&snapshot, APPLICATION_NORITO);
    assert!(
        snapshot
            .headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("content-type")),
        "GET must not carry Content-Type: {:?}",
        snapshot.headers
    );

    let expected_hash = proof.block_header.hash();
    let actual = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);

            client.get_bridge_finality_proof(
                proof.block_header.height(),
                expected_hash,
                &mut verifier,
            )
        },
    )
    .0
    .expect("correct proof must verify after a rejected requested binding");
    assert_eq!(actual, proof);
}

#[test]
fn bridge_finality_next_reader_rejects_height_mismatch_before_advancing() {
    let client = client_with_base_url(base_url());

    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier
        .verify(&anchor)
        .expect("fixture anchor must initialize verifier progress");
    let successor_height = successor.block_header.height();
    let anchor_body = norito::to_bytes(&anchor).expect("encode canonical anchor proof");
    let (error, snapshot) = capture_request(
        mk_response(StatusCode::OK, anchor_body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);

            client
                .get_next_bridge_finality_proof(successor_height, &mut verifier)
                .expect_err("proof from the wrong height must fail")
        },
    );
    assert!(error.to_string().contains("requested height"));
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/2");
    assert!(snapshot.url.query().is_none());

    let successor_body = norito::to_bytes(&successor).expect("encode canonical successor proof");
    let actual = capture_request(
        mk_response(StatusCode::OK, successor_body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_next_bridge_finality_proof(successor_height, &mut verifier)
        },
    )
    .0
    .expect("valid successor must verify after a height mismatch");
    assert_eq!(actual, successor);
}

#[test]
fn bridge_finality_next_reader_response_contract_failures_do_not_advance() {
    let client = client_with_base_url(base_url());

    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier
        .verify(&anchor)
        .expect("fixture anchor must initialize verifier progress");
    let height = successor.block_header.height();
    let body = norito::to_bytes(&successor).expect("encode canonical successor proof");

    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(
            StatusCode::BAD_GATEWAY,
            b"upstream failure".to_vec(),
            Some(APPLICATION_NORITO),
        ),
    );
    assert!(error.contains("Failed to get bridge finality proof"));

    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_JSON)),
    );
    assert!(error.contains("invalid content-type"));

    let mut duplicate_content_type =
        mk_response(StatusCode::OK, body.clone(), Some(APPLICATION_NORITO));
    duplicate_content_type.headers_mut().append(
        "content-type",
        APPLICATION_NORITO.parse().expect("Norito media type"),
    );
    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        duplicate_content_type,
    );
    assert!(error.contains("multiple Content-Type"));

    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(
            StatusCode::OK,
            vec![0; BRIDGE_FINALITY_PROOF_RESPONSE_MAX_BYTES + 1],
            Some(APPLICATION_NORITO),
        ),
    );
    assert!(error.contains("response exceeds"));

    let mut trailing = body.clone();
    trailing.push(0);
    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(StatusCode::OK, trailing, Some(APPLICATION_NORITO)),
    );
    assert!(error.contains("canonical Norito"));

    let actual = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_next_bridge_finality_proof(height, &mut verifier)
        },
    )
    .0
    .expect("valid successor must verify after rejected responses");
    assert_eq!(actual, successor);
}

#[test]
fn bridge_finality_next_reader_verification_failure_does_not_advance() {
    let client = client_with_base_url(base_url());

    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier
        .verify(&anchor)
        .expect("fixture anchor must initialize verifier progress");
    let height = successor.block_header.height();
    let mut invalid = successor.clone();
    invalid.finality_artifact.commit_qc.aggregate_signature[0] ^= 0x40;
    let invalid_body = norito::to_bytes(&invalid).expect("encode invalid successor finality proof");
    let error = rejected_next_bridge_finality_response(
        &client,
        height,
        &mut verifier,
        mk_response(StatusCode::OK, invalid_body, Some(APPLICATION_NORITO)),
    );
    assert!(error.contains("verification failed"));

    let body = norito::to_bytes(&successor).expect("encode canonical successor proof");
    let actual = capture_request(
        mk_response(StatusCode::OK, body, Some(APPLICATION_NORITO)),
        |mock_transport| {
            let client = client
                .clone()
                .with_test_http_transport(mock_transport.clone());
            mark_data_model_compatible(&client);
            client.get_next_bridge_finality_proof(height, &mut verifier)
        },
    )
    .0
    .expect("valid successor must verify after a rejected invalid signature");
    assert_eq!(actual, successor);
}

// These are real signed model fixtures and injected HTTP responses; they do not launch a node.
struct GenesisAttestationFixture {
    attestation: BridgeFinalityAttestationV1,
    signer: KeyPair,
}

impl GenesisAttestationFixture {
    fn new() -> Self {
        use iroha_data_model::{
            block::consensus_v2::finality::V2FinalityArtifact,
            bridge::{BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BridgeFinalityAttestationBodyV1},
        };
        use iroha_model_base::peer::PeerId;
        let (mut proof, _, _) = bridge_finality_chain_fixture();
        let mut keys = (0..4)
            .map(|_| KeyPair::try_random_with_algorithm(Algorithm::BlsNormal).expect("BLS key"))
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let roster = keys
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let network = NetworkId::from_genesis_hash(proof.block_header.hash());
        let (_, mut mint_roster) = mint_finality_authority_fixture(&roster);
        mint_roster.network_id = network;
        let mut context = proof.finality_artifact.height_context.clone();
        context.network_id = network;
        context.mode = ConsensusMode::Npos;
        context.kagemusha_mint_finality_authorization =
            iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1::genesis(&mint_roster, context.epoch_end_height).expect("mint authorization");
        context.kagemusha_mint_finality_authority = mint_roster;
        context.roster = roster;
        context.quorum = DualQuorum::from_roster(&context.roster).expect("four-validator quorum");
        context.validate().expect("independent genesis context");
        let mut qc = proof.finality_artifact.commit_qc.clone();
        qc.round.context_id = context.id();
        qc.proposal_round = qc.round;
        sign_bridge_finality_qc(&mut qc, &keys);
        let pops = keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("BLS PoP"))
            .collect();
        proof.finality_artifact =
            V2FinalityArtifact::new(context, proof.finality_artifact.subject, qc, pops);
        BridgeFinalityVerifier::with_context(network, proof.finality_artifact.context_id())
            .verify(&proof)
            .expect("three genuine BLS votes from four independent validator keys");
        let signer = keys.remove(0);
        let node_id = PeerId::new(signer.public_key().clone());
        let body = BridgeFinalityAttestationBodyV1 {
            version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
            challenge: [0x73; 32],
            network_id: network,
            node_fingerprint: Hash::new(node_id.encode()),
            node_id,
            genesis_block_hash: proof.block_header.hash(),
            status: Self::status(&proof, signer.public_key()),
            genesis_finality_proof: proof.clone(),
            finality_proof: proof,
        };
        let signature =
            iroha_crypto::SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
                .expect("node attestation signature");
        let fixture = Self {
            attestation: BridgeFinalityAttestationV1 { body, signature },
            signer,
        };
        fixture
            .attestation
            .verify()
            .expect("coherent signed node statement");
        fixture
    }

    fn status(
        proof: &BridgeFinalityProof,
        public_key: &iroha_crypto::PublicKey,
    ) -> iroha_data_model::block::consensus_v2::SumeragiV2Status {
        use iroha_data_model::block::consensus_v2::{
            SumeragiV2BodyState, SumeragiV2CommitQcStatus, SumeragiV2HeightContextStatus,
            SumeragiV2LivenessStatus, SumeragiV2Status, SumeragiV2StatusPhase,
        };
        let artifact = &proof.finality_artifact;
        let context = &artifact.height_context;
        let node_id = iroha_model_base::peer::PeerId::new(public_key.clone());
        let status = SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(node_id.encode()),
            build_fingerprint: Hash::new(b"SDK attestation fixture build"),
            config_fingerprint: Hash::new(b"SDK attestation fixture config"),
            restart_required: false,
            height_context_id: context.id(),
            height: artifact.height,
            view: artifact.commit_qc.round.view,
            phase: SumeragiV2StatusPhase::PendingApply,
            leader: context.leader(artifact.commit_qc.round.view),
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Applied,
            pending_persistence_id: None,
            last_committed_height: artifact.height,
            last_committed_subject: Some(artifact.subject),
            height_context: SumeragiV2HeightContextStatus {
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
                mode: context.mode,
                epoch_seed: context.leader_seed,
                validator_count: 4,
                quorum: context.quorum,
            },
            last_commit_qc: Some(SumeragiV2CommitQcStatus {
                certificate: artifact.commit_qc.as_ref(),
                validator_count: 4,
                signer_count: 3,
                min_signers: context.quorum.min_signers,
                signed_power: 3,
                total_power: 4,
            }),
            liveness: SumeragiV2LivenessStatus::default(),
        };
        status.validate().expect("exact authoritative status shape");
        status
    }

    fn client(&self) -> Client {
        let mut builder = client_with_base_url(base_url()).to_builder();
        builder.network_id = self.attestation.body.network_id;
        builder.torii_request_timeout = Duration::from_millis(731);
        builder.build().expect("independently bound client")
    }

    fn resign(&self, attestation: &mut BridgeFinalityAttestationV1) {
        attestation.signature = iroha_crypto::SignatureOf::try_from_hash(
            self.signer.private_key(),
            attestation.body.signing_hash(),
        )
        .expect("resign exact mutated node statement");
    }

    fn read(&self, client: &Client) -> Result<BridgeFinalityAttestationV1> {
        let body = &self.attestation.body;
        client
            .poll_genesis_finality_attestation(
                body.challenge,
                &body.node_id,
                body.network_id,
                body.genesis_block_hash,
                body.genesis_finality_proof.finality_artifact.context_id(),
                std::time::Instant::now() + Duration::from_secs(30),
            )
            .and_then(|outcome| match outcome {
                GenesisFinalityReadiness::Ready(attestation) => Ok(*attestation),
                GenesisFinalityReadiness::NotReady(reason) => {
                    Err(eyre!("unexpected failure: {reason:?}"))
                }
            })
    }

    fn response(&self, response: HttpResponse<Vec<u8>>) -> Result<BridgeFinalityAttestationV1> {
        capture_request(response, |transport| {
            let client = self.client().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            self.read(&client)
        })
        .0
    }

    fn reject(&self, attestation: &BridgeFinalityAttestationV1, error: &str) {
        let wire = norito::to_bytes(attestation).expect("canonical negative statement");
        let failure = self
            .response(mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)))
            .expect_err("mutated attestation must fail")
            .to_string();
        assert!(
            failure.contains(error),
            "expected {error:?}, got {failure:?}"
        );
    }
}

#[test]
fn genesis_attestation_reader_authenticates_real_quorum_and_node() {
    let fixture = GenesisAttestationFixture::new();
    let body = &fixture.attestation.body;
    assert_eq!(
        body.genesis_finality_proof
            .finality_artifact
            .height_context
            .mode,
        ConsensusMode::Npos,
    );
    assert_eq!(
        body.genesis_finality_proof
            .finality_artifact
            .height_context
            .roster
            .len(),
        4
    );
    assert!(
        body.genesis_finality_proof
            .finality_artifact
            .height_context
            .roster
            .iter()
            .any(|validator| validator.validator == body.node_id)
    );
    let wire = norito::to_bytes(&fixture.attestation).expect("canonical signed attestation");
    let (result, snapshot) = capture_request(
        mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)),
        |transport| {
            let client = fixture.client().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            fixture.read(&client)
        },
    );
    assert_eq!(
        result.expect("independent genesis/node authentication"),
        fixture.attestation
    );
    assert_eq!(snapshot.method, HttpMethod::GET);
    assert_eq!(snapshot.url.path(), "/v1/bridge/finality/attestation/1");
    assert!(snapshot.url.query().is_none());
    assert_eq!(
        snapshot.max_response_bytes,
        GENESIS_FINALITY_ATTESTATION_RESPONSE_MAX_BYTES
    );
    assert_eq!(snapshot.timeout, Some(Duration::from_millis(731)));
    assert_single_accept_header(&snapshot, APPLICATION_NORITO);
    let challenge_headers: Vec<_> = snapshot
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(GENESIS_FINALITY_CHALLENGE_HEADER))
        .collect();
    assert_eq!(challenge_headers.len(), 1);
    assert_eq!(challenge_headers[0].1, hex::encode(body.challenge));
}

#[test]
fn genesis_attestation_reader_replaces_every_inherited_challenge_header() {
    let fixture = GenesisAttestationFixture::new();
    let wire = norito::to_bytes(&fixture.attestation).expect("canonical attestation");
    let (result, snapshot) = capture_request(
        mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)),
        |transport| {
            let mut client = fixture.client().with_test_http_transport(transport);
            client.headers.insert(
                "X-Iroha-Finality-Challenge".to_owned(),
                "old-one".to_owned(),
            );
            client.headers.insert(
                "x-iroha-finality-challenge".to_owned(),
                "old-two".to_owned(),
            );
            client
                .headers
                .insert("Accept".to_owned(), APPLICATION_JSON.to_owned());
            client
                .headers
                .insert("Content-Type".to_owned(), APPLICATION_JSON.to_owned());
            mark_data_model_compatible(&client);
            fixture.read(&client)
        },
    );
    result.expect("fresh request header overrides all inherited spellings");
    let challenges: Vec<_> = snapshot
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(GENESIS_FINALITY_CHALLENGE_HEADER))
        .collect();
    assert_eq!(challenges.len(), 1);
    assert_eq!(
        challenges[0].1,
        hex::encode(fixture.attestation.body.challenge)
    );
    assert_single_accept_header(&snapshot, APPLICATION_NORITO);
    assert!(
        snapshot
            .headers
            .iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("content-type"))
    );
}

#[test]
fn genesis_attestation_reader_rejects_invalid_expectations_before_any_request() {
    let fixture = GenesisAttestationFixture::new();
    let body = &fixture.attestation.body;
    let other_hash = HashOf::from_untyped_unchecked(Hash::new(b"other genesis"));
    let other_network = NetworkId::from_genesis_hash(other_hash);
    let non_bls = KeyPair::try_random_with_algorithm(Algorithm::Ed25519).expect("non-BLS key");
    let non_bls_peer = iroha_model_base::peer::PeerId::new(non_bls.public_key().clone());
    for (challenge, peer, network, hash, expected_error) in [
        (
            [0; 32],
            &body.node_id,
            body.network_id,
            body.genesis_block_hash,
            "non-zero",
        ),
        (
            body.challenge,
            &non_bls_peer,
            body.network_id,
            body.genesis_block_hash,
            "BLS-normal",
        ),
        (
            body.challenge,
            &body.node_id,
            other_network,
            other_hash,
            "client/network/genesis",
        ),
        (
            body.challenge,
            &body.node_id,
            body.network_id,
            other_hash,
            "client/network/genesis",
        ),
    ] {
        let (result, snapshots) = capture_requests(empty_response(StatusCode::OK), |transport| {
            let client = fixture.client().with_test_http_transport(transport);
            // Intentionally do not mark compatibility: even its request must be absent.
            client.poll_genesis_finality_attestation(
                challenge,
                peer,
                network,
                hash,
                body.genesis_finality_proof.finality_artifact.context_id(),
                std::time::Instant::now() + Duration::from_secs(30),
            )
        });
        assert!(
            result
                .expect_err("invalid independent binding")
                .to_string()
                .contains(expected_error)
        );
        assert!(
            snapshots.is_empty(),
            "invalid input must precede compatibility and HTTP"
        );
    }
}

#[test]
fn genesis_attestation_reader_runs_existing_compatibility_then_exact_request() {
    let fixture = GenesisAttestationFixture::new();
    let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&snapshots);
    let capabilities = compatible_capabilities_body();
    let wire = norito::to_bytes(&fixture.attestation).expect("canonical attestation");
    let result = with_mock_http(
        move |snapshot| {
            let capabilities_request = snapshot.url.path() == "/v1/node/capabilities";
            captured.lock().expect("snapshot lock").push(snapshot);
            Ok(if capabilities_request {
                json_response(StatusCode::OK, &capabilities)
            } else {
                mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_NORITO))
            })
        },
        |transport| fixture.read(&fixture.client().with_test_http_transport(transport)),
    );
    result.expect("uncached compatibility and attestation");
    let snapshots = snapshots.lock().expect("snapshot lock");
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].url.path(), "/v1/node/capabilities");
    assert_eq!(snapshots[1].url.path(), "/v1/bridge/finality/attestation/1");
}

#[test]
fn genesis_attestation_reader_accepts_active_height_two_with_genesis_tip() {
    use iroha_data_model::block::consensus_v2::{SumeragiV2BodyState, SumeragiV2StatusPhase};
    let fixture = GenesisAttestationFixture::new();
    let mut advanced = fixture.attestation.clone();
    let mut context = advanced
        .body
        .genesis_finality_proof
        .finality_artifact
        .height_context
        .clone();
    context.height = 2;
    context.parent_commit_qc = Some(
        advanced
            .body
            .genesis_finality_proof
            .finality_artifact
            .commit_qc
            .clone(),
    );
    context
        .validate()
        .expect("coherent active successor context");
    advanced.body.status.height_context_id = context.id();
    advanced.body.status.height = 2;
    advanced.body.status.phase = SumeragiV2StatusPhase::AwaitingProposal;
    advanced.body.status.body_state = SumeragiV2BodyState::Missing;
    advanced.body.status.leader = context.leader(advanced.body.status.view);
    fixture.resign(&mut advanced);
    advanced
        .verify()
        .expect("genesis committed while reducer awaits height two");
    let wire = norito::to_bytes(&advanced).expect("canonical advanced reducer status");
    let result = fixture
        .response(mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)))
        .expect("current active height is not the durable tip height");
    assert_eq!(result, advanced);
    assert_eq!(result.body.status.last_committed_height, 1);
    assert_eq!(result.body.status.height, 2);
}

#[test]
fn genesis_attestation_reader_rejects_freshness_and_role_substitution() {
    let fixture = GenesisAttestationFixture::new();
    let mut challenge = fixture.attestation.clone();
    challenge.body.challenge[0] ^= 1;
    fixture.resign(&mut challenge);
    challenge
        .verify()
        .expect("valid node signature over another fresh challenge");
    fixture.reject(&challenge, "challenge mismatch");

    let mut peer = fixture.attestation.clone();
    let wrong_signer =
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal).expect("other node");
    peer.body.node_id = iroha_model_base::peer::PeerId::new(wrong_signer.public_key().clone());
    peer.body.node_fingerprint = Hash::new(peer.body.node_id.encode());
    peer.body.status.node_fingerprint = peer.body.node_fingerprint;
    peer.signature = iroha_crypto::SignatureOf::try_from_hash(
        wrong_signer.private_key(),
        peer.body.signing_hash(),
    )
    .expect("valid wrong-role signature");
    peer.verify().expect("self-consistent wrong node");
    fixture.reject(&peer, "node mismatch");
}

#[test]
fn genesis_attestation_reader_rejects_response_network_and_genesis_substitution() {
    let fixture = GenesisAttestationFixture::new();
    let other_hash = HashOf::from_untyped_unchecked(Hash::new(b"foreign response genesis"));
    let mut network = fixture.attestation.clone();
    network.body.network_id = NetworkId::from_genesis_hash(other_hash);
    fixture.resign(&mut network);
    fixture.reject(&network, "network/genesis mismatch");
    let mut genesis = fixture.attestation.clone();
    genesis.body.genesis_block_hash = other_hash;
    fixture.resign(&mut genesis);
    fixture.reject(&genesis, "network/genesis mismatch");
}

#[test]
fn genesis_attestation_reader_rejects_another_well_formed_context_anchor() {
    let fixture = GenesisAttestationFixture::new();
    let body = &fixture.attestation.body;
    let wrong_context = iroha_data_model::block::consensus_v2::HeightContextId(
        HashOf::from_untyped_unchecked(Hash::new(b"independent wrong context")),
    );
    let wire = norito::to_bytes(&fixture.attestation).expect("valid signed response");
    let (result, _) = capture_request(
        mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)),
        |transport| {
            let client = fixture.client().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            client.poll_genesis_finality_attestation(
                body.challenge,
                &body.node_id,
                body.network_id,
                body.genesis_block_hash,
                wrong_context,
                std::time::Instant::now() + Duration::from_secs(30),
            )
        },
    );
    assert!(
        result
            .expect_err("externally wrong context cannot use returned roster")
            .to_string()
            .contains("genesis finality verification failed")
    );
}

#[test]
fn genesis_attestation_reader_rejects_node_signature_forgery() {
    let fixture = GenesisAttestationFixture::new();
    let mut forged = fixture.attestation.clone();
    let wrong_signer = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal).expect("wrong key");
    forged.signature = iroha_crypto::SignatureOf::try_from_hash(
        wrong_signer.private_key(),
        forged.body.signing_hash(),
    )
    .expect("wrong signer signature");
    assert!(forged.verify().is_err());
    fixture.reject(&forged, "genesis attestation verification failed");
}

#[test]
fn genesis_attestation_reader_rejects_forged_qc_even_with_valid_node_signature() {
    let fixture = GenesisAttestationFixture::new();
    let mut forged = fixture.attestation.clone();
    forged
        .body
        .genesis_finality_proof
        .finality_artifact
        .commit_qc
        .aggregate_signature[0] ^= 0x80;
    forged.body.finality_proof = forged.body.genesis_finality_proof.clone();
    fixture.resign(&mut forged);
    forged
        .verify()
        .expect("node signature and structural QC references alone are insufficient");
    fixture.reject(&forged, "genesis finality verification failed");
}

#[test]
fn genesis_attestation_reader_rejects_forged_validator_pop() {
    let fixture = GenesisAttestationFixture::new();
    let mut forged = fixture.attestation.clone();
    forged
        .body
        .genesis_finality_proof
        .finality_artifact
        .validator_set_pops[0][0] ^= 0x40;
    forged.body.finality_proof = forged.body.genesis_finality_proof.clone();
    fixture.resign(&mut forged);
    forged
        .verify()
        .expect("node signature does not authenticate roster possession");
    fixture.reject(&forged, "genesis finality verification failed");
}

#[test]
fn genesis_attestation_reader_rejects_wrong_proof_header_and_duplicate_proofs() {
    let fixture = GenesisAttestationFixture::new();
    let mut wrong_header = fixture.attestation.clone();
    wrong_header.body.genesis_finality_proof.block_header =
        BlockHeader::new(NonZeroU64::new(1).expect("one"), None, None, 999, 0);
    wrong_header.body.finality_proof = wrong_header.body.genesis_finality_proof.clone();
    fixture.resign(&mut wrong_header);
    fixture.reject(&wrong_header, "genesis attestation verification failed");
    let mut unequal = fixture.attestation.clone();
    unequal
        .body
        .finality_proof
        .finality_artifact
        .commit_qc
        .aggregate_signature[0] ^= 1;
    fixture.resign(&mut unequal);
    fixture.reject(&unequal, "exact height-one durable tip");
}

#[test]
fn genesis_attestation_reader_rejects_later_tip_and_wrong_commit_frontier() {
    let fixture = GenesisAttestationFixture::new();
    let (_, successor, _) = bridge_finality_chain_fixture();
    let mut later = fixture.attestation.clone();
    later.body.status = GenesisAttestationFixture::status(&successor, fixture.signer.public_key());
    later.body.finality_proof = successor;
    fixture.resign(&mut later);
    fixture.reject(&later, "exact height-one durable tip");
    let mut wrong_frontier = fixture.attestation.clone();
    wrong_frontier.body.status.last_committed_height = 2;
    fixture.resign(&mut wrong_frontier);
    fixture.reject(&wrong_frontier, "exact height-one durable tip");
}

#[test]
fn genesis_attestation_reader_rejects_unknown_versions_and_restart_required() {
    let fixture = GenesisAttestationFixture::new();
    let mut version = fixture.attestation.clone();
    version.body.version += 1;
    fixture.resign(&mut version);
    fixture.reject(&version, "genesis attestation verification failed");
    let mut proof_version = fixture.attestation.clone();
    proof_version.body.genesis_finality_proof.version += 1;
    proof_version.body.finality_proof = proof_version.body.genesis_finality_proof.clone();
    fixture.resign(&mut proof_version);
    fixture.reject(&proof_version, "genesis attestation verification failed");
    let mut restart = fixture.attestation.clone();
    restart.body.status.restart_required = true;
    fixture.resign(&mut restart);
    fixture.reject(&restart, "genesis attestation verification failed");
}

#[test]
fn genesis_attestation_reader_rejects_status_media_and_size_contract_failures() {
    let fixture = GenesisAttestationFixture::new();
    let wire = norito::to_bytes(&fixture.attestation).expect("valid response");
    let failure_limit =
        iroha_torii_shared::bridge_attestation::FINALITY_ATTESTATION_FAILURE_MAX_BYTES;
    assert!(
        wire.len() > failure_limit,
        "proof exceeds the bounded failure-envelope budget"
    );
    let oversized_failure = format!("response exceeds the {failure_limit}-byte limit");
    for (response, expected_error) in [
        (
            mk_response(
                StatusCode::SERVICE_UNAVAILABLE,
                wire.clone(),
                Some(APPLICATION_NORITO),
            ),
            oversized_failure.as_str(),
        ),
        (
            mk_response(StatusCode::OK, wire.clone(), Some(APPLICATION_JSON)),
            "invalid content-type",
        ),
        (
            mk_response(StatusCode::OK, wire.clone(), None),
            "invalid content-type",
        ),
        (
            mk_response(StatusCode::OK, Vec::new(), Some(APPLICATION_NORITO)),
            "response body is empty",
        ),
        (
            mk_response(
                StatusCode::OK,
                vec![0; GENESIS_FINALITY_ATTESTATION_RESPONSE_MAX_BYTES + 1],
                Some(APPLICATION_NORITO),
            ),
            "response exceeds",
        ),
    ] {
        let error = fixture
            .response(response)
            .expect_err("specific response contract failure")
            .to_string();
        assert!(
            error.contains(expected_error),
            "expected {expected_error:?}, got {error:?}"
        );
    }
    let mut duplicate = mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO));
    duplicate.headers_mut().append(
        "content-type",
        APPLICATION_NORITO.parse().expect("media type"),
    );
    assert!(
        fixture
            .response(duplicate)
            .expect_err("duplicate media type")
            .to_string()
            .contains("multiple Content-Type")
    );
}

#[test]
fn genesis_attestation_reader_rejects_noncanonical_and_trailing_frames() {
    let fixture = GenesisAttestationFixture::new();
    let mut trailing = norito::to_bytes(&fixture.attestation).expect("valid canonical frame");
    trailing.push(0);
    let bare = fixture.attestation.encode();
    let wrong_root = norito::to_bytes(&fixture.attestation.body).expect("different canonical root");
    for wire in [trailing, bare, wrong_root] {
        let error = fixture
            .response(mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)))
            .expect_err("only exact canonical attestation frame is accepted")
            .to_string();
        assert!(error.contains("canonical Norito"), "{error}");
    }
}

#[test]
fn genesis_attestation_reader_honors_enclosing_decode_allocation_limit() {
    let fixture = GenesisAttestationFixture::new();
    let wire = norito::to_bytes(&fixture.attestation).expect("canonical response");
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 1, 64);
    let result = norito::with_decode_limits_scope(limits, || {
        fixture.response(mk_response(
            StatusCode::OK,
            wire.clone(),
            Some(APPLICATION_NORITO),
        ))
    });
    let error = result
        .expect_err("inner SDK scope cannot reset enclosing allocation budget")
        .to_string();
    assert!(error.contains("canonical Norito"), "{error}");
    assert_eq!(
        fixture
            .response(mk_response(StatusCode::OK, wire, Some(APPLICATION_NORITO)))
            .expect("fresh independently budgeted request remains usable"),
        fixture.attestation
    );
}

#[test]
fn genesis_attestation_reader_propagates_transport_failure_without_fallback() {
    let fixture = GenesisAttestationFixture::new();
    let calls = Arc::new(Mutex::new(0));
    let captured = Arc::clone(&calls);
    let result = with_mock_http(
        move |_snapshot| {
            *captured.lock().expect("calls") += 1;
            Err(eyre!("injected genesis transport failure"))
        },
        |transport| {
            let client = fixture.client().with_test_http_transport(transport);
            mark_data_model_compatible(&client);
            fixture.read(&client)
        },
    );
    assert!(
        result
            .expect_err("transport must propagate")
            .to_string()
            .contains("injected genesis transport failure")
    );
    assert_eq!(*calls.lock().expect("calls"), 1);
}

fn poll_genesis_fixture_response(
    fixture: &GenesisAttestationFixture,
    response: HttpResponse<Vec<u8>>,
) -> Result<GenesisFinalityReadiness> {
    capture_request(response, |transport| {
        let client = fixture.client().with_test_http_transport(transport);
        mark_data_model_compatible(&client);
        let body = &fixture.attestation.body;
        client.poll_genesis_finality_attestation(
            body.challenge,
            &body.node_id,
            body.network_id,
            body.genesis_block_hash,
            body.genesis_finality_proof.finality_artifact.context_id(),
            std::time::Instant::now() + Duration::from_secs(30),
        )
    })
    .0
}

fn finality_failure_wire(
    failure: &iroha_torii_shared::bridge_attestation::FinalityAttestationFailure,
) -> Vec<u8> {
    norito::to_bytes(&failure.clone().into_error_envelope()).unwrap()
}

#[test]
fn genesis_readiness_preserves_all_explicit_failure_reasons() {
    use iroha_torii_shared::bridge_attestation::{
        FinalityAttestationFailure, FinalityAttestationFailureReason::*,
    };
    let fixture = GenesisAttestationFixture::new();
    for reason in [
        ConsensusUninitialized,
        GenesisUncommitted,
        RestartRequired,
        TipChanged,
        ConflictingState,
        FinalityUnavailable,
        InternalFailure,
    ] {
        let failure = FinalityAttestationFailure {
            challenge: fixture.attestation.body.challenge,
            height: 1,
            reason,
            tip_mismatch: (reason == TipChanged).then(|| {
                iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1 {
                    requested_height: 1,
                    applied_height: 2,
                    status_height: 2,
                    challenge: fixture.attestation.body.challenge,
                    node_id: fixture.attestation.body.node_id.clone(),
                    network_id: fixture.attestation.body.network_id,
                }
            }),
        };
        let response = mk_response(
            StatusCode::from_u16(reason.http_status_code()).unwrap(),
            finality_failure_wire(&failure),
            Some(APPLICATION_NORITO),
        );
        assert!(
            matches!(poll_genesis_fixture_response(&fixture, response).unwrap(), GenesisFinalityReadiness::NotReady(actual) if actual == reason)
        );
    }
}

#[test]
fn bridge_finality_reader_retries_only_backpressure_within_original_deadline() {
    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier.verify(&anchor).expect("anchor");
    let expected = successor.clone();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observations = Arc::clone(&calls);
    let started = std::time::Instant::now();
    let deadline = started + Duration::from_secs(10);
    let result = with_mock_http(
        move |request| {
            let mut observations = observations.lock().expect("observations");
            observations.push((std::time::Instant::now(), request));
            if observations.len() == 1 {
                let mut response = empty_response(StatusCode::TOO_MANY_REQUESTS);
                response
                    .headers_mut()
                    .insert("retry-after", "1".parse().unwrap());
                Ok(response)
            } else {
                Ok(norito_response(StatusCode::OK, &expected))
            }
        },
        |transport| {
            let client = client_with_base_url(base_url())
                .with_test_http_transport(transport)
                .with_request_deadline(deadline);
            mark_data_model_compatible(&client);
            client.get_next_bridge_finality_proof(successor.block_header.height(), &mut verifier)
        },
    )
    .expect("bounded retry accepts exact successor");
    assert_eq!(result, successor);
    let calls = calls.lock().expect("observations");
    assert_eq!(calls.len(), 2);
    assert!(calls[1].0.duration_since(calls[0].0) >= Duration::from_secs(1));
    for (_, request) in calls.iter() {
        assert_eq!(request.method, HttpMethod::GET);
        assert_eq!(request.url.path(), "/v1/bridge/finality/2");
        assert!(request.timeout.unwrap() <= deadline.duration_since(started));
    }
    assert!(calls[1].1.timeout.unwrap() < calls[0].1.timeout.unwrap());
}

#[test]
fn bridge_finality_reader_rejects_unbounded_or_invalid_backpressure_without_advancing() {
    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier.verify(&anchor).expect("anchor");
    let height = successor.block_header.height();
    // A missing operation deadline, malformed/duplicate hints, an excessive delay,
    // and non-429 errors must all make exactly one dispatch without changing trust.
    for (status, hints, budget) in [
        (StatusCode::TOO_MANY_REQUESTS, vec!["0"], None),
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec!["-1"],
            Some(Duration::from_secs(5)),
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec!["0", "1"],
            Some(Duration::from_secs(5)),
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec!["18446744073709551615"],
            Some(Duration::from_secs(5)),
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec!["1"],
            Some(Duration::from_millis(500)),
        ),
        (
            StatusCode::UNAUTHORIZED,
            vec!["0"],
            Some(Duration::from_secs(5)),
        ),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            vec!["0"],
            Some(Duration::from_secs(5)),
        ),
    ] {
        let mut response = empty_response(status);
        for hint in hints {
            response
                .headers_mut()
                .append("retry-after", hint.parse().unwrap());
        }
        let (result, _) = capture_request(response, |transport| {
            let client = client_with_base_url(base_url()).with_test_http_transport(transport);
            let client = budget.map_or_else(
                || client.clone(),
                |budget| client.with_request_deadline(std::time::Instant::now() + budget),
            );
            mark_data_model_compatible(&client);
            client.get_next_bridge_finality_proof(height, &mut verifier)
        });
        assert!(result.is_err(), "{status} must fail");
    }
    let actual = capture_request(norito_response(StatusCode::OK, &successor), |transport| {
        let client = client_with_base_url(base_url()).with_test_http_transport(transport);
        mark_data_model_compatible(&client);
        client.get_next_bridge_finality_proof(height, &mut verifier)
    })
    .0
    .expect("all failed reads retained the original chain anchor");
    assert_eq!(actual, successor);
}

#[test]
fn activation_evidence_backpressure_preserves_challenge_and_response_bounds() {
    for hint in [None, Some("0")] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let observations = Arc::clone(&calls);
        let challenge = [0x73; 32];
        with_mock_http(
            move |request| {
                let mut observations = observations.lock().unwrap();
                observations.push(request);
                if observations.len() == 1 {
                    let mut response = empty_response(StatusCode::TOO_MANY_REQUESTS);
                    if let Some(hint) = hint {
                        response
                            .headers_mut()
                            .insert("retry-after", hint.parse().unwrap());
                    }
                    Ok(response)
                } else {
                    Ok(empty_response(StatusCode::CONFLICT))
                }
            },
            |transport| {
                let client = client_with_base_url(base_url())
                    .with_test_http_transport(transport)
                    .with_request_deadline(std::time::Instant::now() + Duration::from_secs(5));
                let result = client
                    .send_activation_evidence_read(
                        "/v1/bridge/finality/2/attestation",
                        2048,
                        Some(challenge),
                        ActivationEvidenceReadAuth::Public,
                    )
                    .expect("read-only retry");
                assert_eq!(result.status(), StatusCode::CONFLICT);
            },
        );
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        for request in calls.iter() {
            assert_eq!(request.method, HttpMethod::GET);
            assert_eq!(request.max_response_bytes, 2048);
            let challenges: Vec<_> = request
                .headers
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case("x-iroha-finality-challenge"))
                .collect();
            assert_eq!(challenges.len(), 1);
            assert_eq!(challenges[0].1, hex::encode(challenge));
        }
    }
}

#[test]
fn genesis_readiness_rejects_bare_404_and_503_instead_of_polling() {
    let fixture = GenesisAttestationFixture::new();
    for status in [
        StatusCode::NOT_FOUND,
        StatusCode::SERVICE_UNAVAILABLE,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::BAD_GATEWAY,
    ] {
        assert!(
            poll_genesis_fixture_response(&fixture, mk_response(status, Vec::new(), None)).is_err()
        );
    }
}

#[test]
fn genesis_readiness_rejects_wrong_request_wrong_status_and_unsigned_ready() {
    use iroha_torii_shared::bridge_attestation::{
        FinalityAttestationFailure, FinalityAttestationFailureReason as Reason,
    };
    let fixture = GenesisAttestationFixture::new();
    let correct = FinalityAttestationFailure {
        challenge: fixture.attestation.body.challenge,
        height: 1,
        reason: Reason::GenesisUncommitted,
        tip_mismatch: None,
    };
    let mut challenge = correct.clone();
    challenge.challenge = [44; 32];
    let mut height = correct.clone();
    height.height = 2;
    let mut reason = correct.clone();
    reason.reason = Reason::TipChanged;
    for failure in [challenge, height, reason] {
        assert!(
            poll_genesis_fixture_response(
                &fixture,
                mk_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    finality_failure_wire(&failure),
                    Some(APPLICATION_NORITO)
                )
            )
            .is_err()
        );
    }
    assert!(
        poll_genesis_fixture_response(
            &fixture,
            mk_response(
                StatusCode::OK,
                finality_failure_wire(&correct),
                Some(APPLICATION_NORITO)
            )
        )
        .is_err()
    );
}

#[test]
fn genesis_readiness_rejects_malformed_oversized_and_noncanonical_failure_envelopes() {
    use iroha_torii_shared::bridge_attestation::{
        FINALITY_ATTESTATION_FAILURE_MAX_BYTES, FinalityAttestationFailure,
        FinalityAttestationFailureReason as Reason,
    };
    let fixture = GenesisAttestationFixture::new();
    let value = FinalityAttestationFailure {
        challenge: fixture.attestation.body.challenge,
        height: 1,
        reason: Reason::GenesisUncommitted,
        tip_mismatch: None,
    };
    let wire = finality_failure_wire(&value);
    let mut trailing = wire.clone();
    trailing.push(0);
    for body in [
        vec![0; FINALITY_ATTESTATION_FAILURE_MAX_BYTES + 1],
        vec![1, 2, 3],
        trailing,
    ] {
        assert!(
            poll_genesis_fixture_response(
                &fixture,
                mk_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    body,
                    Some(APPLICATION_NORITO)
                )
            )
            .is_err()
        );
    }
    assert!(
        poll_genesis_fixture_response(
            &fixture,
            mk_response(
                StatusCode::SERVICE_UNAVAILABLE,
                wire.clone(),
                Some(APPLICATION_JSON)
            )
        )
        .is_err()
    );
    let mut duplicate = mk_response(
        StatusCode::SERVICE_UNAVAILABLE,
        wire,
        Some(APPLICATION_NORITO),
    );
    duplicate
        .headers_mut()
        .append("content-type", APPLICATION_NORITO.parse().unwrap());
    assert!(poll_genesis_fixture_response(&fixture, duplicate).is_err());
}

#[test]
fn genesis_readiness_error_decode_honors_the_enclosing_allocation_limit() {
    use iroha_torii_shared::bridge_attestation::{
        FinalityAttestationFailure, FinalityAttestationFailureReason as Reason,
    };
    let fixture = GenesisAttestationFixture::new();
    let wire = finality_failure_wire(&FinalityAttestationFailure {
        challenge: fixture.attestation.body.challenge,
        height: 1,
        reason: Reason::GenesisUncommitted,
        tip_mismatch: None,
    });
    let result = norito::with_decode_limits_scope(norito::DecodeLimits::new(1, 1, 1, 1, 1), || {
        poll_genesis_fixture_response(
            &fixture,
            mk_response(
                StatusCode::SERVICE_UNAVAILABLE,
                wire,
                Some(APPLICATION_NORITO),
            ),
        )
    });
    assert!(result.is_err());
}

#[test]
fn genesis_readiness_expired_deadline_precedes_compatibility_and_transport() {
    let fixture = GenesisAttestationFixture::new();
    let body = &fixture.attestation.body;
    let (result, requests) = capture_requests(empty_response(StatusCode::OK), |transport| {
        let client = fixture.client().with_test_http_transport(transport);
        client.poll_genesis_finality_attestation(
            body.challenge,
            &body.node_id,
            body.network_id,
            body.genesis_block_hash,
            body.genesis_finality_proof.finality_artifact.context_id(),
            std::time::Instant::now(),
        )
    });
    assert!(
        result
            .expect_err("expired deadline")
            .to_string()
            .contains("deadline elapsed")
    );
    assert!(requests.is_empty());
}

#[test]
fn genesis_readiness_deadline_helper_rejects_elapsed_and_accepts_remaining_budget() {
    assert!(Client::ensure_genesis_readiness_deadline(std::time::Instant::now()).is_err());
    assert!(
        Client::ensure_genesis_readiness_deadline(
            std::time::Instant::now() + Duration::from_secs(60)
        )
        .is_ok()
    );
}

#[test]
fn genesis_readiness_cannot_extend_the_original_context_deadline() {
    let fixture = GenesisAttestationFixture::new();
    let body = &fixture.attestation.body;
    let (result, requests) = capture_requests(empty_response(StatusCode::OK), |transport| {
        let client = fixture
            .client()
            .with_test_http_transport(transport)
            .with_request_deadline(std::time::Instant::now());
        client.poll_genesis_finality_attestation(
            body.challenge,
            &body.node_id,
            body.network_id,
            body.genesis_block_hash,
            body.genesis_finality_proof.finality_artifact.context_id(),
            std::time::Instant::now() + Duration::from_secs(60),
        )
    });
    assert!(
        result
            .expect_err("original deadline already expired")
            .to_string()
            .contains("deadline elapsed")
    );
    assert!(requests.is_empty());
}

#[test]
fn genesis_readiness_rejects_unbound_tip_payload_and_never_retries_valid_tip_change() {
    use iroha_torii_shared::{
        bridge_attestation::{
            FinalityAttestationFailure, FinalityAttestationFailureReason as Reason,
        },
        bridge_finality::BridgeFinalityAttestationTipMismatchV1,
    };
    let fixture = GenesisAttestationFixture::new();
    let body = &fixture.attestation.body;
    let progress = BridgeFinalityAttestationTipMismatchV1 {
        requested_height: 1,
        applied_height: 2,
        status_height: 2,
        challenge: body.challenge,
        node_id: body.node_id.clone(),
        network_id: body.network_id,
    };
    let valid = FinalityAttestationFailure {
        challenge: body.challenge,
        height: 1,
        reason: Reason::TipChanged,
        tip_mismatch: Some(progress.clone()),
    };
    let response = |failure: &FinalityAttestationFailure| {
        mk_response(
            StatusCode::CONFLICT,
            finality_failure_wire(failure),
            Some(APPLICATION_NORITO),
        )
    };
    assert!(matches!(
        poll_genesis_fixture_response(&fixture, response(&valid)).unwrap(),
        GenesisFinalityReadiness::NotReady(Reason::TipChanged)
    ));
    assert_eq!(Reason::TipChanged.readiness_state(), "conflict");
    let mut missing = valid.clone();
    missing.tip_mismatch = None;
    let mut node = valid.clone();
    node.tip_mismatch.as_mut().unwrap().node_id = iroha_model_base::peer::PeerId::new(
        KeyPair::try_from_seed(vec![92; 32], Algorithm::BlsNormal)
            .unwrap()
            .public_key()
            .clone(),
    );
    let mut network = valid.clone();
    network.tip_mismatch.as_mut().unwrap().network_id = NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(Hash::new(b"foreign readiness progress")),
    );
    let mut inner_challenge = valid.clone();
    inner_challenge.tip_mismatch.as_mut().unwrap().challenge = [0; 32];
    let mut mixed = valid.clone();
    mixed.reason = Reason::ConflictingState;
    for invalid in [missing, node, network, inner_challenge, mixed] {
        assert!(poll_genesis_fixture_response(&fixture, response(&invalid)).is_err());
    }
}

#[test]
fn bridge_finality_reader_expired_deadline_does_not_dispatch_or_advance() {
    let (anchor, successor, mut verifier) = bridge_finality_chain_fixture();
    verifier.verify(&anchor).expect("anchor");
    with_mock_http(
        |_| panic!("expired deadline must not dispatch"),
        |transport| {
            let client = client_with_base_url(base_url())
                .with_test_http_transport(transport)
                .with_request_deadline(std::time::Instant::now());
            mark_data_model_compatible(&client);
            assert!(
                client
                    .get_next_bridge_finality_proof(successor.block_header.height(), &mut verifier)
                    .is_err()
            );
            assert!(
                client
                    .verify_activation_evidence_successor(&successor, &mut verifier)
                    .is_err()
            );
        },
    );
    verifier
        .verify(&successor)
        .expect("deadline retained original anchor");
}
