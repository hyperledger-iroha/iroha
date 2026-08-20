#[derive(norito::codec::Decode, norito::codec::Encode)]
struct MutableGenesisBlockWire {
    signatures: std::collections::BTreeSet<BlockSignature>,
    payload: BlockPayload,
    result: Option<BlockResult>,
}

fn canonical_executed_genesis_fixture() -> SignedBlock {
    use iroha_data_model::prelude::*;
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
    let transaction = TransactionBuilder::new_genesis(
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let mut block = SignedBlock::genesis(
        vec![transaction],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );
    let entrypoint_hashes = block
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<Vec<_>>();
    block
        .set_transaction_results(
            Vec::new(),
            &entrypoint_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("genesis fixture entrypoint and result must align");
    let final_signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(), block.hash())
            .expect("sign canonical result-bearing genesis fixture"),
    );
    block
        .replace_signatures(std::collections::BTreeSet::from([final_signature]))
        .expect("replace canonical result-bearing genesis signature");
    {
        let mut final_signatures = block.signatures();
        let final_signature = final_signatures
            .next()
            .expect("canonical result-bearing genesis signature");
        assert_eq!(final_signature.index(), 0);
        assert!(final_signatures.next().is_none());
        final_signature
            .signature()
            .verify_hash(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key(), block.hash())
            .expect("verify canonical result-bearing genesis signature");
    }
    block
}

fn mutate_genesis_result(
    block: &SignedBlock,
    mutate: impl FnOnce(&mut BlockPayload, &mut BlockResult),
) -> SignedBlock {
    use norito::codec::DecodeAll as _;
    let encoded = norito::codec::Encode::encode(block);
    let mut wire = MutableGenesisBlockWire::decode_all(&mut encoded.as_slice())
        .expect("canonical genesis fixture must decode into mutable wire parts");
    let result = wire
        .result
        .as_mut()
        .expect("canonical genesis fixture must carry execution results");
    mutate(&mut wire.payload, result);
    let encoded = norito::codec::Encode::encode(&wire);
    SignedBlock::decode_all(&mut encoded.as_slice())
        .expect("adversarial genesis fixture must remain structurally decodable")
}

#[test]
fn check_genesis_block_requires_canonical_execution_results() {
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID;
    let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let canonical = canonical_executed_genesis_fixture();
    assert_eq!(check_genesis_block(&canonical, &genesis_account), Ok(()));

    let resultless = canonical.canonical_resultless_proposal();
    assert_eq!(
        check_genesis_block(&resultless, &genesis_account),
        Err(InvalidGenesisError::MissingResults)
    );

    let zero_result_sentinel = mutate_genesis_result(&canonical, |_, result| {
        *result = BlockResult::default();
    });
    assert_eq!(
        check_genesis_block(&zero_result_sentinel, &genesis_account),
        Err(InvalidGenesisError::ResultCountMismatch {
            expected: 1,
            actual: 0,
        })
    );

    let mut rejected_result = canonical.clone();
    let rejection = TransactionResultInner::Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted("genesis rejection fixture".to_owned()),
        ),
    );
    assert!(rejected_result.update_transaction_result(0, &rejection));
    assert_eq!(
        check_genesis_block(&rejected_result, &genesis_account),
        Err(InvalidGenesisError::ContainsErrors)
    );

    let entrypoint_cache_mismatch = mutate_genesis_result(&canonical, |_, result| {
        result.merkle = MerkleTree::default();
    });
    assert_eq!(
        check_genesis_block(&entrypoint_cache_mismatch, &genesis_account),
        Err(InvalidGenesisError::EntrypointMerkleCacheMismatch)
    );

    let result_cache_mismatch = mutate_genesis_result(&canonical, |_, result| {
        result.result_merkle = MerkleTree::default();
    });
    assert_eq!(
        check_genesis_block(&result_cache_mismatch, &genesis_account),
        Err(InvalidGenesisError::ResultMerkleCacheMismatch)
    );

    let header_result_root_mismatch = mutate_genesis_result(&canonical, |payload, _| {
        payload.header.result_merkle_root = None;
    });
    assert_eq!(
        check_genesis_block(&header_result_root_mismatch, &genesis_account),
        Err(InvalidGenesisError::ResultMerkleMismatch)
    );

    let mut extra_internal_fragments = canonical.clone();
    extra_internal_fragments.set_committed_fragment_count(3);
    assert_eq!(
        check_genesis_block(&extra_internal_fragments, &genesis_account),
        Ok(()),
        "deterministic internal fragments may increase the committed count beyond the result count"
    );

    let mut committed_count_too_small = canonical;
    committed_count_too_small.set_committed_fragment_count(0);
    assert_eq!(
        check_genesis_block(&committed_count_too_small, &genesis_account),
        Err(
            InvalidGenesisError::CommittedFragmentCountBelowResultCount {
                minimum: 1,
                actual: Some(0),
            }
        )
    );
}

// The executor upgrade is optional; a genesis without it must still pass static checks.
#[test]
fn resultless_genesis_without_upgrade_authenticates_intents() {
    use iroha_data_model::prelude::*;
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
    let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let tx = TransactionBuilder::new_genesis(
        genesis_account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let block = SignedBlock::genesis(
        vec![tx],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );
    assert!(authenticate_genesis_block_intents(&block, &genesis_account).is_ok());
}
#[test]
fn check_genesis_block_rejects_proof_policy_sidecar_substitution() {
    use iroha_data_model::prelude::*;
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
    let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let tx = TransactionBuilder::new_genesis(
        genesis_account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let mut block = SignedBlock::genesis(
        vec![tx],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );
    let signed_header = block.header();
    block.set_da_proof_policies(Some(
        iroha_data_model::da::commitment::DaProofPolicyBundle::new(Vec::new()),
    ));
    block.replace_header_for_testing(signed_header);
    assert_eq!(
        check_genesis_block(&block, &genesis_account),
        Err(InvalidGenesisError::DaProofPolicyMismatch)
    );
}
#[test]
fn check_genesis_block_rejects_height_above_one() {
    use iroha_data_model::prelude::*;
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
    let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let tx = TransactionBuilder::new_genesis(
        genesis_account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let mut block = SignedBlock::genesis(
        vec![tx],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );
    let mut header = block.header();
    header.set_height(nonzero!(2_u64));
    block.replace_header_for_testing(header);
    let signature = BlockSignature::new(
        0,
        checked_block_signature(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(), block.hash()),
    );
    block
        .replace_signatures([signature].into_iter().collect())
        .expect("replace signature after changing test header");
    assert_eq!(
        check_genesis_block(&block, &genesis_account),
        Err(InvalidGenesisError::InvalidHeader)
    );
}
